use std::{
	any::type_name,
	borrow::{Borrow, BorrowMut},
	cell::{Ref, RefMut},
	fmt,
	marker::PhantomData,
	mem::{self, transmute},
	num::NonZeroU64,
	ops::{Deref, DerefMut},
	sync::{
		atomic::{AtomicU64, Ordering},
		Arc, Weak,
	},
};

use derive_where::derive_where;
use hashbrown::HashSet;
use parking_lot::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::{
	debug::{
		label::{DebugLabel, ReifiedDebugLabel},
		lifetime::{DebugLifetime, Dependent, LifetimeLike},
		userdata::{DebugOpaque, Userdata},
	},
	lang::{
		loan::{BorrowingMutexGuard, BorrowingRwReadGuard, BorrowingRwWriteGuard, Mapped},
		marker::PhantomInvariant,
	},
	mem::{
		drop_guard::DropOwnedGuard, eventual_map::EventualMap, ptr::PointeeCastExt,
		type_map::TypeMap,
	},
};

use super::{
	context::{Provider, UnpackTarget},
	entity::{Archetype, ArchetypeId, ArchetypeSet},
	event::{EventQueue, EventQueueIter, TaskQueue},
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
	task_queue: Mutex<TaskQueue<UniverseTask>>,
	destruction_list: Arc<DestructionList>,
}

#[derive(Debug)]
struct ArchetypeInner {
	archetype: Mutex<Archetype<()>>,
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

struct UniverseTask {
	name: ReifiedDebugLabel,
	handler: Box<dyn FnMut(&mut Universe) + Send + Sync>,
}

impl fmt::Debug for UniverseTask {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("UniverseTask")
			.field("name", &self.name)
			.finish_non_exhaustive()
	}
}

type UniverseEventHandler<E> = DebugOpaque<fn(&mut Universe, EventQueueIter<E>)>;

impl Universe {
	pub fn new() -> Self {
		Self::default()
	}

	// === Archetype Management === //

	pub fn create_archetype<M: ?Sized>(&self, name: impl DebugLabel) -> ArchetypeHandle<M> {
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
			_ty: PhantomData,
			id,
			destruction_list: Arc::downgrade(&self.destruction_list),
		}
	}

	pub fn archetype_by_id(&self, id: ArchetypeId) -> &Mutex<Archetype> {
		&self.archetypes[&id].archetype
	}

	pub fn add_archetype_meta<T: Userdata>(&self, id: ArchetypeId, value: T) {
		self.archetypes[&id].meta.add(value);
		self.dirty_archetypes.lock().insert(id);
	}

	pub fn add_archetype_handler<E: Userdata>(
		&self,
		id: ArchetypeId,
		handler: fn(&mut Universe, EventQueueIter<E>),
	) {
		self.add_archetype_meta::<UniverseEventHandler<E>>(id, DebugOpaque::new(handler));
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

	pub fn resource<T: BuildableResource>(&self) -> &T {
		self.resources.get_or_create(|| T::create(self))
	}

	pub fn resource_rw<T: BuildableResourceRw>(&self) -> &RwLock<T> {
		self.resource()
	}

	pub fn archetype<T: ?Sized + BuildableArchetypeBundle>(&self) -> &Mutex<Archetype<()>> {
		let id = self.resource::<UniverseArchetypeResource<T>>().id();
		self.archetype_by_id(id)
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
		F: 'static + Send + Sync + FnOnce(&mut Universe),
	{
		let mut handler = Some(handler);

		self.task_queue.lock().push(UniverseTask {
			name: name.reify(),
			handler: Box::new(move |universe| (handler.take().unwrap())(universe)),
		});
	}

	pub fn queue_event_dispatch<E: Userdata>(&self, mut events: EventQueue<E>) {
		self.queue_task(
			format_args!("EventQueue<{}> dispatch", type_name::<E>()),
			move |universe| {
				for iter in events.flush_all() {
					let arch = iter.arch();
					let handler = *universe.archetype_meta::<UniverseEventHandler<E>>(arch);
					handler(universe, iter);
				}
			},
		);
	}

	// === Management === //

	pub fn dispatch_tasks(&mut self) {
		while let Some(mut task) = self.task_queue.get_mut().next_task() {
			log::trace!("Executing universe task {:?}", task.name);
			(task.handler)(self);
		}

		self.task_queue.get_mut().clear_capacities();
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

#[derive_where(Debug)]
#[repr(C)]
pub struct ArchetypeHandle<M: ?Sized = ()> {
	_ty: PhantomInvariant<M>,
	id: ArchetypeId,
	destruction_list: Weak<DestructionList>,
}

impl<M: ?Sized> ArchetypeHandle<M> {
	pub fn id(&self) -> ArchetypeId {
		self.id
	}

	pub fn cast_marker<N: ?Sized>(self) -> ArchetypeHandle<N> {
		unsafe {
			// Safety: This struct is `repr(C)` and `N` is only ever used in a `PhantomData`.
			transmute(self)
		}
	}

	pub fn cast_marker_ref<N: ?Sized>(&self) -> &ArchetypeHandle<N> {
		unsafe {
			// Safety: This struct is `repr(C)` and `N` is only ever used in a `PhantomData`.
			self.transmute_pointee_ref()
		}
	}

	pub fn cast_marker_mut<N: ?Sized>(&mut self) -> &mut ArchetypeHandle<N> {
		unsafe {
			// Safety: This struct is `repr(C)` and `N` is only ever used in a `PhantomData`.
			self.transmute_pointee_mut()
		}
	}
}

impl<M: ?Sized> Borrow<ArchetypeId> for ArchetypeHandle<M> {
	fn borrow(&self) -> &ArchetypeId {
		&self.id
	}
}

impl<M: ?Sized> Drop for ArchetypeHandle<M> {
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

// === Resource traits === //

pub trait BuildableResource: Userdata {
	fn create(universe: &Universe) -> Self;
}

pub trait BuildableResourceRw: Userdata {
	fn create(universe: &Universe) -> Self;
}

impl<T: BuildableResourceRw> BuildableResource for RwLock<T> {
	fn create(universe: &Universe) -> Self {
		RwLock::new(T::create(universe))
	}
}

pub trait BuildableArchetypeBundle: 'static {
	fn create_archetype(universe: &Universe) -> ArchetypeHandle<Self> {
		universe.create_archetype(type_name::<Self>())
	}
}

#[derive_where(Debug)]
pub struct UniverseArchetypeResource<T: ?Sized>(pub ArchetypeHandle<T>);

impl<T: ?Sized + BuildableArchetypeBundle> BuildableResource for UniverseArchetypeResource<T> {
	fn create(universe: &Universe) -> Self {
		Self(T::create_archetype(universe))
	}
}

impl<T: ?Sized> UniverseArchetypeResource<T> {
	pub fn id(&self) -> ArchetypeId {
		self.0.id()
	}
}

// === Resource dependency injection in `Provider` === //

pub struct Res<T>(PhantomInvariant<T>);

pub struct ResRw<T>(PhantomInvariant<T>);

pub struct ResArch<T: ?Sized>(PhantomInvariant<T>);

impl<'guard: 'borrow, 'borrow> UnpackTarget<'guard, 'borrow, Universe> for &'borrow Universe {
	type Guard = &'guard Universe;
	type Reference = &'borrow Universe;

	fn acquire_guard(src: &'guard Universe) -> Self::Guard {
		src
	}

	fn acquire_ref(guard: &'borrow mut Self::Guard) -> Self::Reference {
		guard
	}
}

impl<'guard: 'borrow, 'borrow, T: BuildableResource> UnpackTarget<'guard, 'borrow, Universe>
	for Res<&'borrow T>
{
	type Guard = &'guard T;
	type Reference = &'borrow T;

	fn acquire_guard(src: &'guard Universe) -> Self::Guard {
		src.resource()
	}

	fn acquire_ref(guard: &'borrow mut Self::Guard) -> Self::Reference {
		guard
	}
}

impl<'guard: 'borrow, 'borrow, T: BuildableResourceRw> UnpackTarget<'guard, 'borrow, Universe>
	for ResRw<&'borrow T>
{
	type Guard = RwLockReadGuard<'guard, T>;
	type Reference = &'borrow T;

	fn acquire_guard(src: &'guard Universe) -> Self::Guard {
		src.resource_rw().try_read().unwrap()
	}

	fn acquire_ref(guard: &'borrow mut Self::Guard) -> Self::Reference {
		&*guard
	}
}

impl<'guard: 'borrow, 'borrow, T: BuildableResourceRw> UnpackTarget<'guard, 'borrow, Universe>
	for ResRw<&'borrow mut T>
{
	type Guard = RwLockWriteGuard<'guard, T>;
	type Reference = &'borrow mut T;

	fn acquire_guard(src: &'guard Universe) -> Self::Guard {
		src.resource_rw().try_write().unwrap()
	}

	fn acquire_ref(guard: &'borrow mut Self::Guard) -> Self::Reference {
		&mut *guard
	}
}

impl<'guard: 'borrow, 'borrow, T: ?Sized + BuildableArchetypeBundle>
	UnpackTarget<'guard, 'borrow, Universe> for ResArch<T>
{
	type Guard = MutexGuard<'guard, Archetype>;
	type Reference = &'borrow mut Archetype<T>;

	fn acquire_guard(src: &'guard Universe) -> Self::Guard {
		src.archetype::<T>().try_lock().unwrap()
	}

	fn acquire_ref(guard: &'borrow mut Self::Guard) -> Self::Reference {
		guard.cast_marker_mut()
	}
}

impl<'provider, 'guard: 'borrow, 'borrow, T: BuildableResource>
	UnpackTarget<'guard, 'borrow, Provider<'provider>> for Res<&'borrow T>
{
	type Guard = ProviderResourceGuard<'guard, T>;
	type Reference = &'borrow T;

	fn acquire_guard(src: &'guard Provider<'provider>) -> Self::Guard {
		if let Some(value) = src.try_get::<T>() {
			return ProviderResourceGuard(value);
		}

		ProviderResourceGuard(Ref::map(src.get::<Universe>(), |universe| {
			universe.resource::<T>()
		}))
	}

	fn acquire_ref(guard: &'borrow mut Self::Guard) -> Self::Reference {
		&*guard
	}
}

impl<'provider, 'guard: 'borrow, 'borrow, T: BuildableResourceRw>
	UnpackTarget<'guard, 'borrow, Provider<'provider>> for ResRw<&'borrow T>
{
	type Guard = ProviderResourceRefGuard<'guard, T>;
	type Reference = &'borrow T;

	fn acquire_guard(src: &'guard Provider<'provider>) -> Self::Guard {
		if let Some(value) = src.try_get::<T>() {
			return ProviderResourceRefGuard::Local(value);
		}

		let res = src.get::<Universe>();
		let res = Ref::map(res, |universe| universe.resource_rw());

		ProviderResourceRefGuard::Universe(BorrowingRwReadGuard::try_new(res).unwrap())
	}

	fn acquire_ref(guard: &'borrow mut Self::Guard) -> Self::Reference {
		&*guard
	}
}

impl<'provider, 'guard: 'borrow, 'borrow, T: BuildableResourceRw>
	UnpackTarget<'guard, 'borrow, Provider<'provider>> for ResRw<&'borrow mut T>
{
	type Guard = ProviderResourceMutGuard<'guard, T>;
	type Reference = &'borrow mut T;

	fn acquire_guard(src: &'guard Provider<'provider>) -> Self::Guard {
		if let Some(value) = src.try_get_mut::<T>() {
			return ProviderResourceMutGuard::Local(value);
		}

		let res = src.get::<Universe>();
		let res = Ref::map(res, |universe| universe.resource_rw());

		ProviderResourceMutGuard::Universe(BorrowingRwWriteGuard::try_new(res).unwrap())
	}

	fn acquire_ref(guard: &'borrow mut Self::Guard) -> Self::Reference {
		&mut *guard
	}
}

impl<'provider, 'guard: 'borrow, 'borrow, T: ?Sized + BuildableArchetypeBundle>
	UnpackTarget<'guard, 'borrow, Provider<'provider>> for ResArch<T>
{
	type Guard = ProviderResourceArchGuard<'guard, T>;
	type Reference = &'borrow mut Archetype<T>;

	fn acquire_guard(src: &'guard Provider<'provider>) -> Self::Guard {
		if let Some(value) = src.try_get_mut::<Archetype<T>>() {
			return ProviderResourceArchGuard::Local(value);
		}

		let res = src.get::<Universe>();
		let res = Ref::map(res, |universe| universe.archetype::<T>());

		ProviderResourceArchGuard::Universe(BorrowingMutexGuard::try_new(res).unwrap())
	}

	fn acquire_ref(guard: &'borrow mut Self::Guard) -> Self::Reference {
		guard.cast_marker_mut()
	}
}

#[derive(Debug)]
pub struct ProviderResourceGuard<'a, T>(Ref<'a, T>);

impl<T> Clone for ProviderResourceGuard<'_, T> {
	fn clone(&self) -> Self {
		Self(Ref::clone(&self.0))
	}
}

impl<T> Deref for ProviderResourceGuard<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T> Borrow<T> for ProviderResourceGuard<'_, T> {
	fn borrow(&self) -> &T {
		&self
	}
}

#[derive(Debug)]
pub enum ProviderResourceRefGuard<'a, T: 'static> {
	Local(Ref<'a, T>),
	Universe(Mapped<Ref<'a, RwLock<T>>, BorrowingRwReadGuard<T>>),
}

impl<T: 'static> Deref for ProviderResourceRefGuard<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		match self {
			Self::Local(r) => &r,
			Self::Universe(r) => &r,
		}
	}
}

impl<T> Borrow<T> for ProviderResourceRefGuard<'_, T> {
	fn borrow(&self) -> &T {
		&self
	}
}

#[derive(Debug)]
pub enum ProviderResourceMutGuard<'a, T: 'static> {
	Local(RefMut<'a, T>),
	Universe(Mapped<Ref<'a, RwLock<T>>, BorrowingRwWriteGuard<T>>),
}

impl<T: 'static> Deref for ProviderResourceMutGuard<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		match self {
			Self::Local(r) => &r,
			Self::Universe(r) => &r,
		}
	}
}

impl<T: 'static> DerefMut for ProviderResourceMutGuard<'_, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		match self {
			Self::Local(r) => &mut *r,
			Self::Universe(r) => &mut *r,
		}
	}
}

impl<T> Borrow<T> for ProviderResourceMutGuard<'_, T> {
	fn borrow(&self) -> &T {
		&self
	}
}

impl<T> BorrowMut<T> for ProviderResourceMutGuard<'_, T> {
	fn borrow_mut(&mut self) -> &mut T {
		&mut *self
	}
}

#[derive(Debug)]
pub enum ProviderResourceArchGuard<'a, T: ?Sized + 'static> {
	Local(RefMut<'a, Archetype<T>>),
	Universe(Mapped<Ref<'a, Mutex<Archetype>>, BorrowingMutexGuard<Archetype>>),
}

impl<T: ?Sized> Deref for ProviderResourceArchGuard<'_, T> {
	type Target = Archetype<T>;

	fn deref(&self) -> &Self::Target {
		match self {
			Self::Local(r) => &r,
			Self::Universe(r) => r.cast_marker_ref(),
		}
	}
}

impl<T: ?Sized> DerefMut for ProviderResourceArchGuard<'_, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		match self {
			Self::Local(r) => &mut *r,
			Self::Universe(r) => r.cast_marker_mut(),
		}
	}
}

impl<T: ?Sized> Borrow<Archetype<T>> for ProviderResourceArchGuard<'_, T> {
	fn borrow(&self) -> &Archetype<T> {
		&self
	}
}

impl<T: ?Sized> BorrowMut<Archetype<T>> for ProviderResourceArchGuard<'_, T> {
	fn borrow_mut(&mut self) -> &mut Archetype<T> {
		&mut *self
	}
}
