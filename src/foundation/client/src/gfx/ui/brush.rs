use std::{
	any::{Any, TypeId},
	cell::{RefCell, RefMut},
	fmt,
	hash::{BuildHasher, Hash, Hasher},
	marker::PhantomData,
};

use crucible_util::{
	lang::{marker::PhantomInvariant, tuple::ToOwnedTupleEq},
	mem::hash::FxHashMap,
};
use derive_where::derive_where;

// === ImmMaterial === //

pub trait ImmMaterial: Sized + 'static + fmt::Debug {
	type Config: 'static + fmt::Debug + Hash + Eq;
	type Layer: 'static + fmt::Debug;

	fn new(config: &mut Self::Config) -> Self;

	fn make_layer(&mut self) -> Self::Layer;
}

#[derive_where(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct ImmMaterialHandle<T> {
	_ty: PhantomInvariant<T>,
	index: usize,
}

// === ImmRenderer === //

#[derive(Debug, Default)]
pub struct ImmRenderer {
	inner: RefCell<ImmRendererInner>,
}

impl ImmRenderer {
	pub fn brush(&self) -> ImmBrush<'_> {
		ImmBrush { renderer: self }
	}

	pub fn use_inner(&mut self) -> RefMut<'_, ImmRendererInner> {
		self.inner.borrow_mut()
	}
}

#[derive(Debug, Copy, Clone)]
pub struct ImmBrush<'a> {
	pub renderer: &'a ImmRenderer,
}

// === ImmRendererInner === //

#[derive(Debug, Default)]
pub struct ImmRendererInner {
	depth: u16,
	material_slot_map: FxHashMap<MaterialKey, usize>,
	materials: Vec<Box<dyn ReifiedMaterial>>,
	instance_layers: Vec<Vec<Option<Box<dyn Any>>>>,
}

impl ImmRendererInner {
	pub fn use_material<T: ImmMaterial>(
		&mut self,
		config: impl ToOwnedTupleEq<OwnedEq = T::Config>,
	) -> ImmMaterialHandle<T> {
		// Construct a hash for our material type and our config type
		let hash = {
			let mut hasher = self.material_slot_map.hasher().build_hasher();

			// Hash the material TypeId to avoid type confusion
			TypeId::of::<T>().hash(&mut hasher);

			// Hash the config
			config.hash(&mut hasher);

			hasher.finish()
		};

		// Lookup the entry in the map.
		let entry = self
			.material_slot_map
			.raw_entry_mut()
			.from_hash(hash, |candidate| {
				if candidate.config_hash != hash {
					return false;
				}

				let Some((_, candidate_config)) = candidate
				.config_and_phantom
				.downcast_ref::<(PhantomInvariant<T>, T::Config)>()
			else {
				return false;
			};

				config.is_eq_owned(&candidate_config)
			});

		// Fill in the entry and return an ID
		match entry {
			hashbrown::hash_map::RawEntryMut::Occupied(entry) => ImmMaterialHandle {
				_ty: PhantomData,
				index: *entry.into_mut(),
			},
			hashbrown::hash_map::RawEntryMut::Vacant(entry) => {
				// Reify config
				let mut config = config.to_owned();

				// Register material
				let index = self.materials.len();
				self.materials.push(Box::new(T::new(&mut config)));

				// Insert slot
				entry.insert_with_hasher(
					hash,
					MaterialKey {
						config_hash: hash,
						config_and_phantom: Box::<MaterialConfigAndPhantom<T>>::new((
							PhantomData,
							config,
						)),
					},
					index,
					|entry| entry.config_hash,
				);

				// Return handle
				ImmMaterialHandle {
					_ty: PhantomData,
					index,
				}
			}
		}
	}

	pub fn get_material<T: ImmMaterial>(&mut self, material: ImmMaterialHandle<T>) -> &mut T {
		self.materials[material.index]
			.as_any_mut()
			.downcast_mut::<T>()
			.unwrap()
	}

	pub fn push_instance<T: ImmMaterial>(
		&mut self,
		material: ImmMaterialHandle<T>,
	) -> (&mut T, &mut T::Layer, u16) {
		// Get material
		let material_data = self.materials[material.index]
			.as_any_mut()
			.downcast_mut::<T>()
			.unwrap();

		// Allocate depth
		let depth = self.depth;
		self.depth = self.depth.wrapping_add(1);
		if depth == 0 {
			self.instance_layers.push(Vec::new());
		}

		// Allocate layer
		let layers = self.instance_layers.last_mut().unwrap();

		if material.index >= layers.len() {
			layers.resize_with(material.index + 1, Default::default);
		}

		let layer = &mut layers[material.index];
		let layer = layer
			.get_or_insert_with(|| Box::new(material_data.make_layer()))
			.downcast_mut::<T::Layer>()
			.unwrap();

		// Return material, layer, and depth
		(material_data, layer, depth)
	}

	// TODO: Rendering
}

// Internals
type MaterialConfigAndPhantom<T> = (PhantomData<T>, <T as ImmMaterial>::Config);

#[derive(Debug)]
struct MaterialKey {
	config_hash: u64,
	config_and_phantom: Box<dyn Any>,
}

trait ReifiedMaterial: 'static + fmt::Debug {
	fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: ImmMaterial> ReifiedMaterial for T {
	fn as_any_mut(&mut self) -> &mut dyn Any {
		self
	}
}
