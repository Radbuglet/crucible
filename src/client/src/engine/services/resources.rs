use std::{
	any::Any,
	cell::Cell,
	collections::hash_map::RandomState,
	error::Error,
	fmt, hash,
	time::{Duration, Instant},
};

use crucible_common::util::linked_list::ObjLinkedList;
use hashbrown::raw::RawTable;

use crucible_core::{
	error::ResultExt, hasher::hash_one, iter::ContextualIter, linked_list::LinkedList,
};
use geode::prelude::*;
use once_cell::unsync::OnceCell;

// === ResourceDescriptor === //

component_bundle! {
	pub struct ResourceBundle<T>(ResourceBundleCtor) {
		resource: T,
	}
}

pub trait ResourceDescriptor<C>: fmt::Debug + ObjPointee + hash::Hash + Eq + Sync {
	type Resource: ObjPointee;
	type Error: Error;

	fn create(
		&self,
		s: Session,
		res_mgr: &mut ResourceManager,
		ctx: C,
	) -> Result<Owned<ResourceBundle<Self::Resource>>, Self::Error>;

	fn keep_alive(&self, _s: Session, _res_mgr: &mut ResourceManager, _ctx: C) {}
}

// === ResourceManager === //

type ResListLink<'s> = &'s Cell<Option<Obj<ResourceEntry>>>;
type TouLinkedList<'s> = ObjLinkedList<
	's,
	Obj<ResourceEntry>,
	fn(Session<'s>, Obj<ResourceEntry>) -> (ResListLink<'s>, ResListLink<'s>),
>;

fn get_res_list_view<'s>(
	session: Session<'s>,
	head: ResListLink<'s>,
	tail: ResListLink<'s>,
) -> TouLinkedList<'s> {
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
	resource_map: RawTable<Owned<Obj<ResourceEntry>>>,

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
	hash: u64,

	// === Resource state === //
	descriptor: Owned<Obj<dyn Any + Send>>,
	value: OnceCell<Owned<Entity>>,

	// === TOU state === //
	tou_left: Cell<Option<Obj<ResourceEntry>>>,
	tou_right: Cell<Option<Obj<ResourceEntry>>>,
	tou_time: Cell<Instant>,
}

impl ResourceManager {
	pub fn new(lock: Lock) -> Self {
		Self {
			resource_map: Default::default(),
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
		let (hash, entry) = self.fetch_entry_inner(s, &descriptor);

		if let Some(entry) = entry {
			// Fetch the resource
			let p_entry = entry.get(s);
			let resource = p_entry
				.value
				.get()
				.unwrap_or_else(|| panic!("cannot load a resource that is currently being loaded"))
				.weak_copy();

			// Update the TOU
			let mut entries = get_res_list_view(
				s,
				Cell::from_mut(&mut self.tou_head),
				Cell::from_mut(&mut self.tou_tail),
			);

			entries.unlink(Some(entry));
			entries.insert_head(Some(entry));
			p_entry.tou_time.set(Instant::now());

			// Run the descriptor's keep alive handlers.
			descriptor.keep_alive(s, self, ctx);

			Ok(ResourceBundle::cast(resource))
		} else {
			// Box the descriptor.
			let (descriptor_guard, descriptor) = descriptor.box_obj(s).to_guard_ref_pair();

			// Box the entry
			let (entry_guard, entry) = ResourceEntry {
				hash,
				descriptor: descriptor_guard.unsize(),
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
				entry_guard,
				|entry| entry.get(s).hash
			);

			// Register it in the TOU linked list.
			let mut entries = get_res_list_view(
				s,
				Cell::from_mut(&mut self.tou_head),
				Cell::from_mut(&mut self.tou_tail),
			);

			// N.B. we do this after the resource map insertion since the former is more panic prone
			// and, if we panic there, the registry should be placed in a valid state.
			entries.insert_head(Some(entry));

			// Release `RefCell` and create the resource.
			let created = descriptor.get(s).create(s, self, ctx);
			log::info!("Loaded resource with descriptor {descriptor:?} from scratch.");

			// Register the new resource and return it
			match created {
				Ok(resource_guard) => {
					// Register resource
					let resource = resource_guard.weak_copy();

					let _ = entry.get(s).value.set(resource_guard.raw());

					Ok(resource)
				}
				Err(err) => {
					self.unregister_resource(s, entry);
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

	pub fn keep_alive<C, D: ResourceDescriptor<C>>(
		&mut self,
		s: Session,
		ctx: C,
		descriptor: D,
	) -> bool {
		// Ensure that the resource is loaded.
		let (_, entry) = self.fetch_entry_inner(s, &descriptor);

		if let Some(entry) = entry {
			// Push it to the front of the TOU list.
			let mut entries = get_res_list_view(
				s,
				Cell::from_mut(&mut self.tou_head),
				Cell::from_mut(&mut self.tou_tail),
			);

			entries.unlink(Some(entry));
			entries.insert_head(Some(entry));
			entry.get(s).tou_time.set(Instant::now());

			// And run its corresponding handlers.
			descriptor.keep_alive(s, self, ctx);
			true
		} else {
			false
		}
	}

	pub fn collect_unused(&mut self, s: Session, max_tou: Duration) {
		let mut entries = get_res_list_view(
			s,
			Cell::from_mut(&mut self.tou_head),
			Cell::from_mut(&mut self.tou_tail),
		);

		let mut iter = entries.iter_backwards_interactive();
		let now = Instant::now();

		while let Some(entry) = iter.next(&entries) {
			let entry = entry.unwrap();
			let p_entry = entry.get(s);

			let elapsed = now.duration_since(p_entry.tou_time.get());

			if elapsed > max_tou {
				Self::unregister_resource_inner(s, &mut entries, &mut self.resource_map, entry);
				log::info!(
					"Freed resource (entity: {:?}) that hadn't been used for {:?}",
					p_entry.value.get(),
					elapsed
				);
			}
		}
	}

	pub fn get_lock(&self) -> Lock {
		self.lock
	}

	// === Internals === //

	fn fetch_entry_inner<C, D: ResourceDescriptor<C>>(
		&self,
		s: Session,
		descriptor: &D,
	) -> (u64, Option<Obj<ResourceEntry>>) {
		let hash = hash_one(&self.hash_builder, &descriptor);

		let entry = self.resource_map.get(hash, |entry| {
			if hash != entry.get(s).hash {
				return false;
			}

			let entry = entry.get(s);

			if !matches!(
				entry.descriptor.get(s).downcast_ref::<D>(),
				Some(rhs_descriptor) if descriptor == rhs_descriptor
			) {
				return false;
			}

			true
		});

		(hash, entry.map(|val| val.weak_copy()))
	}

	fn unregister_resource_inner(
		s: Session,
		entries: &mut TouLinkedList,
		resource_map: &mut RawTable<Owned<Obj<ResourceEntry>>>,
		entry: Obj<ResourceEntry>,
	) {
		// Remove from the linked list
		// N.B. we do this first because `resource_map.remove_entry` is more panic prone than we are
		// and we'd like to keep the registry in as much of a valid state as possible.
		entries.unlink(Some(entry));

		// Remove from the map.
		#[rustfmt::skip]
		let _ = resource_map.remove_entry(
			entry.get(s).hash,
			|other_entry| other_entry.weak_copy() == entry
		);
	}

	fn unregister_resource(&mut self, s: Session, entry: Obj<ResourceEntry>) {
		Self::unregister_resource_inner(
			s,
			&mut get_res_list_view(
				s,
				Cell::from_mut(&mut self.tou_head),
				Cell::from_mut(&mut self.tou_tail),
			),
			&mut self.resource_map,
			entry,
		);
	}
}
