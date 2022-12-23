use std::{
	any::type_name,
	borrow::Borrow,
	mem,
	num::NonZeroU64,
	sync::{
		atomic::{AtomicU64, Ordering},
		Arc, Weak,
	},
};

use hashbrown::HashSet;
use parking_lot::{Mutex, MutexGuard, RwLockReadGuard, RwLockWriteGuard};

use crate::{
	debug::{
		label::DebugLabel,
		lifetime::{DebugLifetime, Dependent, LifetimeLike},
		userdata::{DebugOpaque, Userdata},
	},
	mem::{drop_guard::DropOwnedGuard, eventual_map::EventualMap, type_map::TypeMap},
};

use super::{
	entity::{Archetype, ArchetypeId, ArchetypeSet},
	event::{EventQueue, EventQueueIter, TaskQueue},
	provider::{DynProvider, Provider},
	storage::{CelledStorage, Storage},
};

// === Universe === //

#[derive(Debug, Default)]
pub struct Universe {
	archetypes: EventualMap<ArchetypeId, ArchetypeInner>,
	tags: EventualMap<NonZeroU64, TagInner>,
	tag_alloc: AtomicU64,
	dirty_archetypes: Mutex<HashSet<ArchetypeId>>,
	task_queue: Mutex<TaskQueue>,
	resources: TypeMap,
	destruction_list: Arc<DestructionList>,
}

#[derive(Debug)]
struct ArchetypeInner {
	archetype: Mutex<Archetype>,
	meta: TypeMap,
	tags: Mutex<Vec<NonZeroU64>>,
}

#[derive(Debug)]
struct TagInner {
	_lifetime: DropOwnedGuard<DebugLifetime>,
	tagged: Mutex<ArchetypeSet>,
}

#[derive(Debug, Default)]
struct DestructionList {
	archetypes: Mutex<Vec<ArchetypeId>>,
	tags: Mutex<Vec<TagId>>,
}

type UniverseEventHandler<E> = DebugOpaque<fn(&Universe, EventQueueIter<E>)>;

impl Universe {
	pub fn new() -> Self {
		Self::default()
	}

	// === Archetype Management === //

	pub fn create_archetype(&self, name: impl DebugLabel) -> ArchetypeHandle {
		let archetype = Archetype::new(name);
		let id = archetype.id();
		self.archetypes.create(
			id,
			Box::new(ArchetypeInner {
				archetype: Mutex::new(archetype),
				meta: Default::default(),
				tags: Default::default(),
			}),
		);

		ArchetypeHandle {
			id,
			destruction_list: Arc::downgrade(&self.destruction_list),
		}
	}

	pub fn archetype(&self, id: ArchetypeId) -> MutexGuard<Archetype> {
		self.archetypes[&id].archetype.lock()
	}

	pub fn add_archetype_meta<T: Userdata>(&self, id: ArchetypeId, value: T) {
		self.archetypes[&id].meta.create(value);
		self.dirty_archetypes.lock().insert(id);
	}

	pub fn add_archetype_handler<E: Userdata>(
		&self,
		id: ArchetypeId,
		handler: fn(&Universe, EventQueueIter<E>),
	) {
		self.add_archetype_meta(id, DebugOpaque::new(handler));
	}

	pub fn try_get_archetype_meta<T: Userdata>(&self, id: ArchetypeId) -> Option<&T> {
		self.archetypes[&id].meta.try_get()
	}

	pub fn archetype_meta<T: Userdata>(&self, id: ArchetypeId) -> &T {
		self.archetypes[&id].meta.get()
	}

	// === Archetype Tagging === //

	pub fn create_tag(&self, name: impl DebugLabel) -> TagHandle {
		let id = NonZeroU64::new(self.tag_alloc.fetch_add(1, Ordering::Relaxed)).unwrap();
		let lifetime = DebugLifetime::new(name);
		self.tags.create(
			id,
			Box::new(TagInner {
				_lifetime: DropOwnedGuard::new(lifetime),
				tagged: Default::default(),
			}),
		);
		TagHandle {
			id: TagId { lifetime, id },
			destruction_list: Arc::downgrade(&self.destruction_list),
		}
	}

	pub fn tag_archetype(&self, arch: ArchetypeId, tag: TagId) {
		let did_insert = self.tags[&tag.id]
			.tagged
			.lock()
			.insert(Dependent::new(arch));

		if did_insert {
			self.archetypes[&arch].tags.lock().push(tag.id);
		}
	}

	pub fn tagged_archetypes(&self, tag: TagId) -> HashSet<ArchetypeId> {
		self.tags[&tag.id]
			.tagged
			.lock()
			.iter()
			.map(|arch| arch.get())
			.collect()
	}

	// === Resource Management === //

	pub fn storage<T: Userdata>(&self) -> RwLockReadGuard<Storage<T>> {
		self.resources.lock_ref_or_create(Default::default)
	}

	pub fn storage_mut<T: Userdata>(&self) -> RwLockWriteGuard<Storage<T>> {
		self.resources.lock_mut_or_create(Default::default)
	}

	pub fn celled_storage<T: Userdata>(&self) -> RwLockReadGuard<CelledStorage<T>> {
		self.resources.lock_ref_or_create(Default::default)
	}

	pub fn celled_storage_mut<T: Userdata>(&self) -> RwLockWriteGuard<CelledStorage<T>> {
		self.resources.lock_mut_or_create(Default::default)
	}

	pub fn resource<T: UniverseResource>(&self) -> &T {
		self.resources.get_or_create(|| T::create_resource(self))
	}

	// === Event Queue === //

	pub fn queue_task<F>(&self, name: impl DebugLabel, handler: F)
	where
		F: 'static + Send + Sync + FnOnce(&Universe),
	{
		self.task_queue
			.lock()
			.push(name, |_tq, cx: &mut DynProvider<'_>| {
				handler((&*cx).get_comp::<Universe>())
			});
	}

	pub fn queue_event_dispatch<E: Userdata>(&self, mut events: EventQueue<E>) {
		self.queue_task(
			format_args!("EventQueue<{}> dispatch", type_name::<E>()),
			move |universe| {
				for iter in events.flush_all() {
					let arch = iter.arch();
					universe.archetype_meta::<UniverseEventHandler<E>>(arch)(universe, iter);
				}
			},
		);
	}

	// === Management === //

	pub fn dispatch_tasks(&mut self) {
		loop {
			let task_queue = self.task_queue.get_mut();
			if task_queue.is_empty() {
				break;
			}

			// Yes, we have two layers of task queue stealing... so what? Replacing vectors isn't
			// *that* expensive.
			let mut task_queue = mem::replace(task_queue, TaskQueue::new());
			task_queue.dispatch(&mut ((&*self).as_dyn()));
		}
	}

	pub fn flush(&mut self) {
		// Flush all `EventualMaps`
		self.archetypes.flush();
		self.tags.flush();
		self.resources.flush();

		for archetype in mem::replace(self.dirty_archetypes.get_mut(), HashSet::new()) {
			self.archetypes[&archetype].meta.flush();
		}

		// TODO: Process destruction requests
	}
}

// === Handles === //

#[derive(Debug)]
pub struct ArchetypeHandle {
	id: ArchetypeId,
	destruction_list: Weak<DestructionList>,
}

impl ArchetypeHandle {
	pub fn id(&self) -> ArchetypeId {
		self.id
	}
}

impl Borrow<ArchetypeId> for ArchetypeHandle {
	fn borrow(&self) -> &ArchetypeId {
		&self.id
	}
}

impl Drop for ArchetypeHandle {
	fn drop(&mut self) {
		let Some(dtor_list) = self.destruction_list.upgrade() else {
			log::error!("Failed to destroy ArchetypeHandle for {:?}: owning universe was destroyed.", self.id);
			return;
		};

		dtor_list.archetypes.lock().push(self.id);
	}
}

#[derive(Debug)]
pub struct TagHandle {
	id: TagId,
	destruction_list: Weak<DestructionList>,
}

impl TagHandle {
	pub fn id(&self) -> TagId {
		self.id
	}
}

impl Borrow<TagId> for TagHandle {
	fn borrow(&self) -> &TagId {
		&self.id
	}
}

impl Drop for TagHandle {
	fn drop(&mut self) {
		let Some(dtor_list) = self.destruction_list.upgrade() else {
			log::error!("Failed to destroy TagHandle for {:?}: owning universe was destroyed.", self.id);
			return;
		};

		dtor_list.tags.lock().push(self.id);
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct TagId {
	lifetime: DebugLifetime,
	id: NonZeroU64,
}

impl LifetimeLike for TagId {
	fn is_possibly_alive(&self) -> bool {
		self.lifetime.is_possibly_alive()
	}

	fn is_condemned(&self) -> bool {
		self.lifetime.is_condemned()
	}

	fn inc_dep(&self) {
		self.lifetime.inc_dep()
	}

	fn dec_dep(&self) {
		self.lifetime.dec_dep()
	}
}

pub trait UniverseResource: Userdata {
	fn create_resource(universe: &Universe) -> Self;
}
