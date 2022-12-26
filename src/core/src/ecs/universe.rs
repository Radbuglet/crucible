use std::{
	any::type_name,
	borrow::Borrow,
	cell::{Ref, RefMut},
	mem,
	num::NonZeroU64,
	ops::{Deref, DerefMut},
	sync::{
		atomic::{AtomicU64, Ordering},
		Arc, Weak,
	},
};

use hashbrown::HashSet;
use parking_lot::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::{
	debug::{
		label::DebugLabel,
		lifetime::{DebugLifetime, Dependent, LifetimeLike},
		userdata::{DebugOpaque, Userdata},
	},
	lang::loan::{BorrowingRwReadGuard, BorrowingRwWriteGuard, Mapped},
	mem::{drop_guard::DropOwnedGuard, eventual_map::EventualMap, type_map::TypeMap},
};

use super::{
	entity::{Archetype, ArchetypeId, ArchetypeSet},
	event::{EventQueue, EventQueueIter, TaskQueue},
	provider::{Provider, ProviderPack},
	storage::{CelledStorage, Storage},
};

// === Universe === //

#[derive(Debug, Default)]
pub struct Universe {
	archetypes: EventualMap<ArchetypeId, ArchetypeInner>,
	tags: EventualMap<TagId, TagInner>,
	tag_alloc: AtomicU64,
	dirty_archetypes: Mutex<HashSet<ArchetypeId>>,
	resources: TypeMap,
	task_queue: Mutex<TaskQueue>,
	destruction_list: Arc<DestructionList>,
}

#[derive(Debug)]
struct ArchetypeInner {
	archetype: Mutex<Archetype>,
	meta: TypeMap,
	tags: Mutex<HashSet<TagId>>,
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
		self.archetypes.add(
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
		self.archetypes[&id].meta.add(value);
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
		let id = NonZeroU64::new(self.tag_alloc.fetch_add(1, Ordering::Relaxed) + 1).unwrap();
		let lifetime = DebugLifetime::new(name);
		let id = TagId { lifetime, id };

		self.tags.add(
			id,
			Box::new(TagInner {
				_lifetime: DropOwnedGuard::new(lifetime),
				tagged: Default::default(),
			}),
		);

		TagHandle {
			id,
			destruction_list: Arc::downgrade(&self.destruction_list),
		}
	}

	pub fn tag_archetype(&self, arch: ArchetypeId, tag: TagId) {
		let did_insert = self.tags[&tag].tagged.lock().insert(Dependent::new(arch));

		if did_insert {
			self.archetypes[&arch].tags.lock().insert(tag);
		}
	}

	pub fn tagged_archetypes(&self, tag: TagId) -> HashSet<ArchetypeId> {
		self.tags[&tag]
			.tagged
			.lock()
			.iter()
			.map(|arch| arch.get())
			.collect()
	}

	// === Resource Management === //

	pub fn resources(&self) -> &TypeMap {
		&self.resources
	}

	pub fn resource<T: UniverseResource>(&self) -> &T {
		self.resources.get_or_create(|| T::create(self))
	}

	pub fn resource_rw<T: UniverseResource>(&self) -> &RwLock<T> {
		self.resources
			.get_or_create(|| RwLock::new(T::create(self)))
	}

	pub fn storage<T: Userdata>(&self) -> RwLockReadGuard<Storage<T>> {
		self.resource_rw().try_read().unwrap()
	}

	pub fn storage_mut<T: Userdata>(&self) -> RwLockWriteGuard<Storage<T>> {
		self.resource_rw().try_write().unwrap()
	}

	pub fn celled_storage<T: Userdata>(&self) -> RwLockReadGuard<CelledStorage<T>> {
		self.resource_rw().try_read().unwrap()
	}

	pub fn celled_storage_mut<T: Userdata>(&self) -> RwLockWriteGuard<CelledStorage<T>> {
		self.resource_rw().try_write().unwrap()
	}

	// === Event Queue === //

	pub fn queue_task<F>(&self, name: impl DebugLabel, handler: F)
	where
		F: 'static + Send + Sync + FnOnce(&Provider<'_>, &Universe),
	{
		self.task_queue.lock().push(name, |_tq, cx: &Provider<'_>| {
			handler(cx, &cx.get::<Universe>());
		});
	}

	pub fn queue_event_dispatch<E: Userdata>(&self, mut events: EventQueue<E>) {
		self.queue_task(
			format_args!("EventQueue<{}> dispatch", type_name::<E>()),
			move |_, universe| {
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
			task_queue.dispatch(&Provider::new().with(&mut *self));
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

		// Flush archetype deletions
		for arch in mem::replace(&mut *self.destruction_list.archetypes.lock(), Vec::new()) {
			// Remove archetype from archetype map
			let arch_info = self.archetypes.remove(&arch).unwrap();

			// Unregister tag dependencies
			for tag in arch_info.tags.into_inner() {
				self.tags[&tag].tagged.get_mut().remove(&arch);
			}

			// (archetype is destroyed on drop)
		}

		// Flush tag deletions
		for tag in mem::replace(&mut *self.destruction_list.tags.lock(), Vec::new()) {
			// Remove tag from tag map
			let tag_info = self.tags.remove(&tag).unwrap();

			// Unregister archetypal dependencies
			for arch in tag_info.tagged.into_inner() {
				self.archetypes[&arch.get()].tags.get_mut().remove(&tag);
			}

			// (lifetime is killed implicitly on drop)
		}
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

// === Resources === //

pub type ResStorageMut<'a, T> = ResMut<'a, Storage<T>>;
pub type ResStorageRef<'a, T> = ResRef<'a, Storage<T>>;
pub type ResCelledStorageMut<'a, T> = ResMut<'a, CelledStorage<T>>;
pub type ResCelledStorageRef<'a, T> = ResRef<'a, CelledStorage<T>>;

pub trait UniverseResource: Userdata {
	fn create(universe: &Universe) -> Self;
}

#[derive(Debug)]
pub struct Res<'a, T>(Ref<'a, T>);

impl<T> Clone for Res<'_, T> {
	fn clone(&self) -> Self {
		Self(Ref::clone(&self.0))
	}
}

impl<T> Deref for Res<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<'a, T: UniverseResource> ProviderPack<'a> for Res<'a, T> {
	fn get_from_provider(provider: &'a Provider) -> Self {
		if let Some(value) = provider.try_get::<T>() {
			return Self(value);
		}

		Self(Ref::map(provider.get::<Universe>(), |universe| {
			universe.resource::<T>()
		}))
	}
}

#[derive(Debug)]
pub enum ResRef<'a, T: 'static> {
	Local(Ref<'a, T>),
	Universe(Mapped<Ref<'a, RwLock<T>>, BorrowingRwReadGuard<T>>),
}

impl<T: 'static> Deref for ResRef<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		match self {
			Self::Local(r) => &r,
			Self::Universe(r) => &r,
		}
	}
}

impl<'a, T: UniverseResource> ProviderPack<'a> for ResRef<'a, T> {
	fn get_from_provider(provider: &'a Provider) -> Self {
		if let Some(value) = provider.try_get::<T>() {
			return Self::Local(value);
		}

		let res = provider.get::<Universe>();
		let res = Ref::map(res, |universe| universe.resource_rw());

		Self::Universe(BorrowingRwReadGuard::try_new(res).unwrap())
	}
}

#[derive(Debug)]
pub enum ResMut<'a, T: 'static> {
	Local(RefMut<'a, T>),
	Universe(Mapped<Ref<'a, RwLock<T>>, BorrowingRwWriteGuard<T>>),
}

impl<T: 'static> Deref for ResMut<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		match self {
			Self::Local(r) => &r,
			Self::Universe(r) => &r,
		}
	}
}

impl<T: 'static> DerefMut for ResMut<'_, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		match self {
			Self::Local(r) => &mut *r,
			Self::Universe(r) => &mut *r,
		}
	}
}

impl<'a, T: UniverseResource> ProviderPack<'a> for ResMut<'a, T> {
	fn get_from_provider(provider: &'a Provider) -> Self {
		if let Some(value) = provider.try_get_mut::<T>() {
			return Self::Local(value);
		}

		let res = provider.get::<Universe>();
		let res = Ref::map(res, |universe| universe.resource_rw());

		Self::Universe(BorrowingRwWriteGuard::try_new(res).unwrap())
	}
}
