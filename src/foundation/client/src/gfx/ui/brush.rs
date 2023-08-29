use std::{
	any::{Any, TypeId},
	cell::{RefCell, RefMut},
	fmt,
	hash::{BuildHasher, Hash, Hasher},
	marker::PhantomData,
};

use crucible_foundation_shared::math::Aabb2;
use crucible_util::{
	lang::{marker::PhantomInvariant, tuple::ToOwnedTupleEq},
	mem::hash::FxHashMap,
};
use derive_where::derive_where;
use typed_glam::glam::{Affine2, Vec2};

use crate::engine::{assets::AssetManager, io::gfx::GfxContext};

// === ImmMaterial === //

#[derive_where(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct ImmMaterialHandle<T> {
	_ty: PhantomInvariant<T>,
	index: usize,
}

pub trait ImmMaterial: Sized + 'static + fmt::Debug {
	type Config: 'static + fmt::Debug + Hash + Eq;
	type Pass<'a>: ImmPass;

	fn new(config: &mut Self::Config) -> Self;

	fn create_layer(&mut self);

	fn create_many_layers(&mut self, count: usize) {
		for _ in 0..count {
			self.create_layer();
		}
	}

	fn create_pass<'a>(
		&'a mut self,
		gfx: &'a GfxContext,
		assets: &mut AssetManager,
		surface_format: wgpu::TextureFormat,
		depth_format: wgpu::TextureFormat,
	) -> Self::Pass<'a>;
}

pub trait ImmPass: fmt::Debug {
	fn render<'a>(&'a self, layer: usize, pass: &mut wgpu::RenderPass<'a>);
}

// === ImmRenderer === //

#[derive(Debug, Default)]
pub struct ImmRenderer {
	inner: RefCell<ImmRendererInner>,
}

impl ImmRenderer {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn brush(&self) -> ImmBrush<'_> {
		ImmBrush {
			renderer: self,
			transform: Affine2::IDENTITY,
		}
	}

	pub fn use_inner(&self) -> RefMut<'_, ImmRendererInner> {
		self.inner.borrow_mut()
	}

	pub fn prepare_render<'a>(
		&'a mut self,
		gfx: &'a GfxContext,
		assets: &mut AssetManager,
		surface_format: wgpu::TextureFormat,
		depth_format: wgpu::TextureFormat,
	) -> ImmRenderPass<'a> {
		self.inner
			.get_mut()
			.prepare_render(gfx, assets, surface_format, depth_format)
	}
}

#[derive(Debug, Copy, Clone)]
pub struct ImmBrush<'a> {
	pub renderer: &'a ImmRenderer,
	pub transform: Affine2,
}

impl<'a> ImmBrush<'a> {
	// Transform
	pub fn transform_before(&mut self, transform: Affine2) {
		self.transform = self.transform * transform;
	}

	#[must_use]
	pub fn transformed_before(mut self, transform: Affine2) -> Self {
		self.transform_before(transform);
		self
	}

	pub fn transform_after(&mut self, transform: Affine2) {
		self.transform = transform * self.transform;
	}

	#[must_use]
	pub fn transformed_after(mut self, transform: Affine2) -> Self {
		self.transform_after(transform);
		self
	}

	// Translate
	pub fn translate_before(&mut self, translation: Vec2) {
		self.transform_before(Affine2::from_translation(translation));
	}

	#[must_use]
	pub fn translated_before(mut self, translation: Vec2) -> Self {
		self.translate_before(translation);
		self
	}

	pub fn translate_after(&mut self, translation: Vec2) {
		self.transform_after(Affine2::from_translation(translation));
	}

	#[must_use]
	pub fn translated_after(mut self, translation: Vec2) -> Self {
		self.translate_after(translation);
		self
	}

	// Scale
	pub fn scale_before(&mut self, scale: Vec2) {
		self.transform_before(Affine2::from_scale(scale));
	}

	#[must_use]
	pub fn scaled_before(mut self, scale: Vec2) -> Self {
		self.scale_before(scale);
		self
	}

	pub fn scale_after(&mut self, scale: Vec2) {
		self.transform_after(Affine2::from_scale(scale));
	}

	#[must_use]
	pub fn scaled_after(mut self, scale: Vec2) -> Self {
		self.scale_after(scale);
		self
	}

	// Transform rectangle
	pub fn transform_rect_after(&mut self, dest: Aabb2<Vec2>, src: Aabb2<Vec2>) {
		// Convert to unit coordinate-space
		self.translate_after(-src.origin);
		self.scale_after(1.0 / src.size);

		// Convert to dest coordinate-space
		self.scale_after(dest.size);
		self.translate_after(dest.origin);
	}

	#[must_use]
	pub fn transformed_rect_after(mut self, from: Aabb2<Vec2>, to: Aabb2<Vec2>) -> Self {
		self.transform_rect_after(from, to);
		self
	}
}

// === ImmRendererInner === //

#[derive(Debug, Default)]
pub struct ImmRendererInner {
	depth: u16,
	layer_count: usize,
	material_slot_map: FxHashMap<MaterialKey, usize>,
	materials: Vec<Box<dyn ReifiedMaterial>>,
}

impl ImmRendererInner {
	pub fn find_material<T: ImmMaterial>(
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
					.downcast_ref::<MaterialConfigAndPhantom<T>>()
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

				// Create material
				let mut material = Box::new(T::new(&mut config));
				material.create_many_layers(self.layer_count);

				// Register material
				let index = self.materials.len();
				self.materials.push(material);

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

	pub fn find_and_get_material<T: ImmMaterial>(
		&mut self,
		config: impl ToOwnedTupleEq<OwnedEq = T::Config>,
	) -> &mut T {
		let id = self.find_material(config);
		self.get_material(id)
	}

	pub fn alloc_depth(&mut self) -> u16 {
		let depth = self.depth;
		self.depth = self.depth.wrapping_add(1);
		if depth == 0 {
			self.layer_count += 1;

			for material in &mut self.materials {
				material.create_layer_dyn();
			}
		}

		depth
	}

	pub fn layer_count(&self) -> usize {
		self.layer_count
	}

	#[must_use]
	pub fn prepare_render<'a>(
		&'a mut self,
		gfx: &'a GfxContext,
		assets: &mut AssetManager,
		surface_format: wgpu::TextureFormat,
		depth_format: wgpu::TextureFormat,
	) -> ImmRenderPass<'a> {
		ImmRenderPass {
			layer_count: self.layer_count,
			materials: self
				.materials
				.iter_mut()
				.map(|material| material.create_pass_dyn(gfx, assets, surface_format, depth_format))
				.collect(),
		}
	}
}

#[derive(Debug)]
pub struct ImmRenderPass<'a> {
	layer_count: usize,
	materials: Vec<Box<dyn ImmPass + 'a>>,
}

impl ImmRenderPass<'_> {
	pub fn render<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
		for layer in 0..self.layer_count {
			for material in &self.materials {
				material.render(layer, pass);
			}
		}
	}
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

	fn create_layer_dyn(&mut self);

	fn create_pass_dyn<'a>(
		&'a mut self,
		gfx: &'a GfxContext,
		assets: &mut AssetManager,
		surface_format: wgpu::TextureFormat,
		depth_format: wgpu::TextureFormat,
	) -> Box<dyn ImmPass + 'a>;
}

impl<T: ImmMaterial> ReifiedMaterial for T {
	fn as_any_mut(&mut self) -> &mut dyn Any {
		self
	}

	fn create_layer_dyn(&mut self) {
		self.create_layer();
	}

	fn create_pass_dyn<'a>(
		&'a mut self,
		gfx: &'a GfxContext,
		assets: &mut AssetManager,
		surface_format: wgpu::TextureFormat,
		depth_format: wgpu::TextureFormat,
	) -> Box<dyn ImmPass + 'a> {
		Box::new(self.create_pass(gfx, assets, surface_format, depth_format))
	}
}
