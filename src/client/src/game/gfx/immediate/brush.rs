use std::{cell::RefCell, marker::PhantomData, mem};

use crucible_common::math::allocate_unit_depth;
use crucible_util::{
	debug::type_id::NamedTypeId, lang::marker::PhantomInvariant, mem::array::vec_from_fn,
};
use hashbrown::HashMap;
use typed_glam::glam;

// === Context === //

#[derive(Debug)]
pub struct ImmContext {
	inner: RefCell<ImmContextInner>,
}

#[derive(Debug, Default)]
struct ImmContextInner {
	z_index: u32,
	material_map: HashMap<NamedTypeId, usize>,
	curr_pass: ImmContextPass,
	prev_passes: Vec<ImmContextPass>,
}

#[derive(Debug, Default)]
struct ImmContextPass {
	buffers: Vec<Vec<u8>>,
}

impl ImmContext {
	pub fn new() -> Self {
		Self {
			inner: Default::default(),
		}
	}
}

// === Brush === //

#[derive(Debug, Clone)]
pub struct ImmBrush<'a> {
	cx: &'a ImmContext,
	transform: glam::Mat3,
}

impl ImmBrush<'_> {
	// === Transforming === //

	pub fn transform(&self) -> glam::Mat3 {
		self.transform
	}

	pub fn set_transform(&mut self, transform: glam::Mat3) -> &mut Self {
		self.transform = transform;
		self
	}

	pub fn apply_transform(&mut self, transform: glam::Mat3) -> &mut Self {
		self.transform = self.transform * transform;
		self
	}

	// === Rendering === //

	pub fn material_id<P: ImmPipeline>(&mut self) -> ImmPipelineId<P> {
		let cx = &mut *self.cx.inner.borrow_mut();
		let index = *cx
			.material_map
			.entry(NamedTypeId::of::<P>())
			.or_insert_with(|| {
				let buffer_index = cx.curr_pass.buffers.len();
				cx.curr_pass.buffers.push(Vec::new());
				buffer_index
			});

		ImmPipelineId {
			_ty: PhantomData,
			index,
		}
	}

	pub fn push<M>(&mut self, id: ImmPipelineId<M::Pipeline>, mat: M) -> &mut Self
	where
		M: ImmObject,
	{
		let cx = &mut *self.cx.inner.borrow_mut();

		// Allocate a depth
		let (depth, needs_new_pass) = allocate_unit_depth(&mut cx.z_index);
		if needs_new_pass {
			let buffer_count = cx.curr_pass.buffers.len();
			cx.prev_passes.push(mem::replace(
				&mut cx.curr_pass,
				ImmContextPass {
					buffers: vec_from_fn(Vec::new, buffer_count),
				},
			));
		}

		// Fetch the material's vertex buffer
		let vertices = &mut cx.curr_pass.buffers[id.index];

		// Write the material
		mat.push(self.transform, depth, vertices);

		self
	}

	pub fn fork(&mut self, f: impl FnOnce(&mut Self)) -> &mut Self {
		f(&mut self.clone());
		self
	}
}

pub trait ImmObject {
	type Pipeline: ImmPipeline;

	fn push(self, transform: glam::Mat3, depth: f32, vertices: &mut Vec<u8>);
}

// === Pipeline === //

#[derive(Debug, Copy, Clone)]
pub struct ImmPipelineId<P> {
	_ty: PhantomInvariant<P>,
	index: usize,
}

pub trait ImmPipeline: 'static {}
