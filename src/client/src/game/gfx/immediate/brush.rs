use std::cell::RefCell;

use bort::OwnedEntity;
use crucible_util::debug::type_id::NamedTypeId;
use hashbrown::HashMap;
use typed_glam::glam;

#[derive(Debug)]
pub struct ImmContext(RefCell<ImmContextInner>);

#[derive(Debug)]
struct ImmContextInner {
	depth: u32,
	pipelines: HashMap<NamedTypeId, OwnedEntity>,
}

#[derive(Debug)]
pub struct ImmPassBase {
	current_buffer: Vec<u8>,
	past_buffers: Vec<Vec<u8>>,
}

#[derive(Debug)]
pub struct ImmBrush<'a> {
	cx: &'a ImmContext,
	transform: glam::Affine2,
}

pub trait ImmPipeline: 'static {
	fn create() -> OwnedEntity;
}

pub trait ImmObject {
	type Pipeline: ImmPipeline;

	fn push(self, transform: glam::Affine2, depth: f32);
}
