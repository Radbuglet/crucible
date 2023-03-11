use bort::CompRef;
use crucible_util::{lang::tuple::PreOwned, mem::array::map_arr};

use crate::engine::{assets::AssetManager, io::gfx::GfxContext};

pub fn load_pipeline_layout<const N: usize, const M: usize>(
	assets: &mut AssetManager,
	gfx: &GfxContext,
	bind_uniforms: [&wgpu::BindGroupLayout; N],
	push_uniforms: [wgpu::PushConstantRange; M],
) -> CompRef<wgpu::PipelineLayout> {
	let bind_ids = map_arr(bind_uniforms, |v| v.global_id());

	assets.cache((&bind_ids, PreOwned(push_uniforms.clone())), move |_| {
		gfx.device
			.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
				label: None,
				bind_group_layouts: &bind_uniforms,
				push_constant_ranges: &push_uniforms,
			})
	})
}
