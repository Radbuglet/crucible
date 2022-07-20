use std::{
	any::Any,
	cell::Cell,
	collections::hash_map::RandomState,
	error::Error,
	hash,
	time::{Duration, Instant},
};

use crucible_common::util::linked_list::ObjLinkedList;
use hashbrown::raw::RawTable;

use crucible_core::{
	c_enum::{c_enum, CEnumMap},
	contextual_iter::ContextualIter,
	error::ResultExt,
	hasher::hash_one,
	linked_list::LinkedList,
};
use geode::prelude::*;
use once_cell::unsync::OnceCell;

// === ManagedResourceAliveQuery === //

pub type ShouldKeepAliveSignal = Signal<dyn EventHandler<ShouldKeepAliveEvent>>;

component_bundle! {
	pub struct ResourceBundle<T>(ResourceBundleCtor) {
		resource: T,
		should_keep_alive: ShouldKeepAliveSignal,
	}
}

#[derive(Debug, Clone, Default)]
pub struct ShouldKeepAliveEvent {
	verdict: KeepAliveVerdict,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Default)]
pub enum KeepAliveVerdict {
	/// This verdict means that no [ManagedResourceAliveQuery] handler has either `testify`'ied or
	/// `condemn`'ed the resource. This is typically taken as an indication that the resource is no
	/// longer needed.
	#[default]
	Undecided,

	/// This verdict means that at least one [ManagedResourceAliveQuery] handler `condemn`'ed the
	/// resource. Condemnation categorically overpowers all testimony supporting the use of a
	/// resource.
	Condemned,

	/// This verdict means that at least one [ManagedResourceAliveQuery] handler `testify`'ied for
	/// this resource's continued use and no other handler overruled that testimony through a
	/// `condemn`'ation.
	Supported,
}

impl KeepAliveVerdict {
	pub fn is_truthy(&self) -> bool {
		match self {
			KeepAliveVerdict::Undecided => false,
			KeepAliveVerdict::Condemned => false,
			KeepAliveVerdict::Supported => true,
		}
	}
}

impl ShouldKeepAliveEvent {
	pub fn testify(&mut self) {
		if self.verdict == KeepAliveVerdict::Undecided {
			self.verdict = KeepAliveVerdict::Supported;
		}
	}

	pub fn condemn(&mut self) {
		self.verdict = KeepAliveVerdict::Condemned;
	}

	pub fn verdict(&self) -> KeepAliveVerdict {
		self.verdict
	}
}

// === CostCategory === //

pub type ResourceCostSet = CEnumMap<CostCategory, u64>;

c_enum! {
	pub enum CostCategory {
		AssetCount,
		CpuMemory,
		GpuMemory,
		GpuTextureCount,
		GpuBufferCount,
		GpuPipelineCount,
	}
}

// === ResourceDescriptor === //

pub struct CreatedResource<R: ?Sized + ObjPointee> {
	pub resource: Owned<ResourceBundle<R>>,
	pub costs: ResourceCostSet,
}

pub trait ResourceDescriptor<C>: ObjPointee + hash::Hash + Eq + Sync {
	type Resource: ObjPointee;
	type Error: Error;

	fn create(
		&self,
		s: Session,
		res_mgr: &mut ResourceManager,
		ctx: C,
	) -> Result<CreatedResource<Self::Resource>, Self::Error>;
}

// === ResourceManager === //

type ResListLink<'s> = &'s Cell<Option<Obj<ResourceEntry>>>;

fn get_res_list_view<'s>(
	session: Session<'s>,
	head: ResListLink<'s>,
	tail: ResListLink<'s>,
) -> ObjLinkedList<
	's,
	Obj<ResourceEntry>,
	fn(Session<'s>, Obj<ResourceEntry>) -> (ResListLink<'s>, ResListLink<'s>),
> {
	ObjLinkedList {
		session,
		head,
		tail,
		access: |s, entry| {
			let entry = entry.get(s);
			(&entry.tou_left, &entry.tou_right)
		},
	}
}

pub struct ResourceManager {
	/// A map from resource descriptors to [ManagedResource] entries. Hashes are cached to prevent
	/// very large amounts of dynamic dispatch during rehashing.
	resource_map: RawTable<(u64, Owned<Obj<ResourceEntry>>)>,

	/// The sum of all the resource's [ResourceCostSet]s.
	total_cost: ResourceCostSet,

	/// The [BuildHasher] for the `resource_map`.
	hash_builder: RandomState,

	/// The head (leftmost element) of the TOU doubly-linked list.
	tou_head: Option<Obj<ResourceEntry>>,

	/// The tail (rightmost element) of the TOU doubly-linked list.
	tou_tail: Option<Obj<ResourceEntry>>,

	/// The lock used by everything `ResourceManager` touches.
	lock: Lock,
}

struct ResourceEntry {
	// === Resource state === //
	descriptor: Owned<Obj<dyn Any + Send>>,
	value: OnceCell<ResourceEntryData>,

	// === TOU state === //
	tou_left: Cell<Option<Obj<ResourceEntry>>>,
	tou_right: Cell<Option<Obj<ResourceEntry>>>,
	tou_time: Cell<Instant>,
}

struct ResourceEntryData {
	resource: Owned<Entity>,
	costs: ResourceCostSet,
}

impl ResourceManager {
	pub fn new(lock: Lock) -> Self {
		Self {
			resource_map: Default::default(),
			total_cost: Default::default(),
			hash_builder: Default::default(),
			tou_head: Default::default(),
			tou_tail: Default::default(),
			lock,
		}
	}

	pub fn try_load<C, D>(
		&mut self,
		s: Session,
		ctx: C,
		descriptor: D,
	) -> Result<ResourceBundle<D::Resource>, D::Error>
	where
		D: ResourceDescriptor<C>,
	{
		// Find existing resource
		let hash = hash_one(&self.hash_builder, &descriptor);

		let entry = self.resource_map.get(hash, |(entry_hash, entry)| {
			if hash != *entry_hash {
				return false;
			}

			let entry = entry.get(s);

			if !matches!(
				entry.descriptor.get(s).downcast_ref::<D>(),
				Some(rhs_descriptor) if &descriptor == rhs_descriptor
			) {
				return false;
			}

			true
		});

		if let Some((_, entry)) = entry {
			// Fetch the resource
			let p_entry = entry.get(s);
			let resource = p_entry
				.value
				.get()
				.unwrap_or_else(|| panic!("cannot load a resource that is currently being loaded"))
				.resource
				.weak_copy();

			// Update the TOU
			get_res_list_view(
				s,
				Cell::from_mut(&mut self.tou_head),
				Cell::from_mut(&mut self.tou_tail),
			)
			.insert_head(Some(entry.weak_copy()));
			p_entry.tou_time.set(Instant::now());

			Ok(ResourceBundle::cast(resource))
		} else {
			// Box the descriptor.
			let (descriptor_guard, descriptor) = descriptor.box_obj(s).to_guard_ref_pair();

			// Box the entry
			let (entry_guard, entry) = ResourceEntry {
				descriptor: descriptor_guard.cast(),
				value: OnceCell::new(),
				tou_time: Cell::new(Instant::now()),
				tou_left: Cell::new(None), // We'll initialize these down below.
				tou_right: Cell::new(None),
			}
			.box_obj_in(s, self.lock)
			.to_guard_ref_pair();

			// Register the resource in the map.
			#[rustfmt::skip]
			self.resource_map.insert(
				hash,
				(hash, entry_guard),
				|(hash, _)| *hash
			);

			// Register it in the TOU linked list.
			// N.B. we do this after the resource map insertion since the former is more panic prone
			// and, if we panic there, the registry should be placed in a valid state.
			get_res_list_view(
				s,
				Cell::from_mut(&mut self.tou_head),
				Cell::from_mut(&mut self.tou_tail),
			)
			.insert_head(Some(entry));

			// Release `RefCell` and create the resource.
			let created = descriptor.get(s).create(s, self, ctx);

			// Register the new resource and return it
			match created {
				Ok(CreatedResource { resource, costs }) => {
					// Update total cost counters
					for (key, cost) in costs.iter() {
						*self.total_cost.entry_mut(key).get_or_insert(0) += *cost;
					}

					// Register resource
					let resource_weak = resource.weak_copy();

					let _ = entry.get(s).value.set(ResourceEntryData {
						resource: resource.raw(),
						costs,
					});

					Ok(resource_weak)
				}
				Err(err) => {
					self.unregister_resource(s, hash, entry);
					Err(err)
				}
			}
		}
	}

	pub fn load<C, D>(&mut self, s: Session, ctx: C, descriptor: D) -> ResourceBundle<D::Resource>
	where
		D: ResourceDescriptor<C>,
	{
		self.try_load(s, ctx, descriptor).unwrap_pretty()
	}

	fn unregister_resource(&mut self, s: Session, hash: u64, entry: Obj<ResourceEntry>) {
		// Decrement cost counter
		if let Some(data) = entry.get(s).value.get() {
			// Update total cost counters
			for (key, cost) in data.costs.iter() {
				*self.total_cost.entry_mut(key).get_or_insert(0) -= *cost;
			}
		}

		// Remove from the linked list
		// N.B. we do this first because `resource_map.remove_entry` is more panic prone than we are
		// and we'd like to keep the registry in as much of a valid state as possible.
		get_res_list_view(
			s,
			Cell::from_mut(&mut self.tou_head),
			Cell::from_mut(&mut self.tou_tail),
		)
		.unlink(Some(entry));

		// Remove from the map.
		#[rustfmt::skip]
		let _ = self.resource_map.remove_entry(
			hash,
			|(_, obj)| obj.weak_copy() == entry
		);
	}

	pub fn collect_garbage(&mut self, s: Session, min_duration: Duration) {
		let entries = get_res_list_view(
			s,
			Cell::from_mut(&mut self.tou_head),
			Cell::from_mut(&mut self.tou_tail),
		);

		let mut iter = entries.iter_backwards_interactive();
		let now = Instant::now();

		while let Some(entry) = iter.next(&entries) {
			let entry = entry.unwrap();
			let p_entry = entry.get(s);

			// Ignore entries that haven't lived long enough.
			if now.duration_since(p_entry.tou_time.get()) < min_duration {
				continue;
			}

			// Check if the resource is still alive.
			let value = match p_entry.value.get() {
				Some(value) => value,
				None => continue,
			};

			let mut should_keep_alive = ShouldKeepAliveEvent::default();

			value
				.resource
				.get::<dyn EventHandler<ShouldKeepAliveEvent>>(s)
				.fire(s, value.resource.weak_copy(), &mut should_keep_alive);

			if should_keep_alive.verdict().is_truthy() {
				continue;
			}

			// The resource is dead!
			// TODO
		}
	}

	pub fn get_lock(&self) -> Lock {
		self.lock
	}

	pub fn make_keep_alive_signal(&self, s: Session) -> Owned<Obj<ShouldKeepAliveSignal>> {
		Signal::new(self.get_lock()).box_obj_in(s, self.get_lock())
	}
}

// === Standard Validator Components === //

// TODO
