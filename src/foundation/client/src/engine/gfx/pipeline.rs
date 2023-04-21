use bort::CompRef;
use crucible_util::{impl_tuples, lang::tuple::PreOwned, mem::array::map_arr};
use typed_wgpu::{
	pipeline::PipelineSet,
	uniform::{BindGroup, BindGroupInstance, BindGroupLayout, PipelineLayout},
};

use crate::engine::{assets::AssetManager, io::gfx::GfxContext};

// === BindGroupExt === //

pub trait BindGroupExt: BindGroup {
	fn load_layout_raw(
		assets: &mut AssetManager,
		gfx: &GfxContext,
		config: &Self::Config,
	) -> CompRef<wgpu::BindGroupLayout> {
		assets.cache(config, |_| Self::create_layout(&gfx.device, config).raw)
	}

	fn load_layout(
		assets: &mut AssetManager,
		gfx: &GfxContext,
		config: &Self::Config,
	) -> CompRef<BindGroupLayout<Self>> {
		CompRef::map(
			Self::load_layout_raw(assets, gfx, config),
			BindGroupLayout::wrap_ref,
		)
	}

	fn load_instance(
		&self,
		assets: &mut AssetManager,
		gfx: &GfxContext,
		config: &Self::Config,
	) -> BindGroupInstance<Self> {
		self.create_instance(
			&gfx.device,
			&Self::load_layout_raw(assets, gfx, config),
			config,
		)
	}
}

impl<T: BindGroup> BindGroupExt for T {}

// === PipelineLayoutExt === //

pub fn load_untyped_pipeline_layout<const N: usize, const M: usize>(
	assets: &mut AssetManager,
	gfx: &GfxContext,
	bind_groups: [&wgpu::BindGroupLayout; N],
	push_constants: [wgpu::PushConstantRange; M],
) -> CompRef<wgpu::PipelineLayout> {
	let bind_ids = map_arr(bind_groups, |v| v.global_id());

	assets.cache((&bind_ids, PreOwned(push_constants.clone())), |_| {
		gfx.device
			.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
				label: None,
				bind_group_layouts: &bind_groups,
				push_constant_ranges: &push_constants,
			})
	})
}

pub trait PipelineLayoutConfigs<K: PipelineSet>: Sized {
	fn load(self, assets: &mut AssetManager, gfx: &GfxContext) -> CompRef<PipelineLayout<K>>;
}

pub trait PipelineLayoutConfigsDefault: PipelineSet {
	fn load_default(assets: &mut AssetManager, gfx: &GfxContext) -> CompRef<PipelineLayout<Self>>;
}

macro_rules! impl_pipeline_layout_configs {
	($($para:ident:$field:tt),*) => {
		impl<$($para: 'static + BindGroup),*> PipelineLayoutConfigs<($($para,)*)> for ($(&$para::Config,)*) {
			fn load(self, assets: &mut AssetManager, gfx: &GfxContext) -> CompRef<PipelineLayout<($($para,)*)>> {
				let bind_groups = [$(&$para::load_layout(assets, gfx, &self.$field).raw),*];
				let push_constants = [];

				let raw = load_untyped_pipeline_layout(assets, gfx, bind_groups, push_constants);

				CompRef::map(raw, PipelineLayout::wrap_ref)
			}
		}

		impl<$($para: 'static + BindGroup),*> PipelineLayoutConfigsDefault for ($($para,)*)
		where
			$($para::Config: Default),*
		{
			fn load_default(assets: &mut AssetManager, gfx: &GfxContext) -> CompRef<PipelineLayout<Self>> {
				($(&<$para::Config>::default(),)*).load(assets, gfx)
			}
		}
	};
}

impl_tuples!(impl_pipeline_layout_configs);

pub trait PipelineLayoutExt {
	type Set: PipelineSet;

	fn load_default(assets: &mut AssetManager, gfx: &GfxContext) -> CompRef<Self>
	where
		Self::Set: PipelineLayoutConfigsDefault;

	fn load(
		assets: &mut AssetManager,
		gfx: &GfxContext,
		configs: impl PipelineLayoutConfigs<Self::Set>,
	) -> CompRef<Self>;
}

impl<T: PipelineSet> PipelineLayoutExt for PipelineLayout<T> {
	type Set = T;

	fn load_default(assets: &mut AssetManager, gfx: &GfxContext) -> CompRef<Self>
	where
		Self::Set: PipelineLayoutConfigsDefault,
	{
		<Self::Set>::load_default(assets, gfx)
	}

	fn load(
		assets: &mut AssetManager,
		gfx: &GfxContext,
		configs: impl PipelineLayoutConfigs<Self::Set>,
	) -> CompRef<Self> {
		configs.load(assets, gfx)
	}
}
