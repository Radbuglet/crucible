use bort::{core::cell::OptRef, CompRef};
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
	) -> OptRef<'static, wgpu::BindGroupLayout> {
		CompRef::into_opt_ref(
			assets.cache(config, |_| Self::create_layout(&gfx.device, config).raw),
		)
	}

	fn load_layout(
		assets: &mut AssetManager,
		gfx: &GfxContext,
		config: &Self::Config,
	) -> OptRef<'static, BindGroupLayout<Self>> {
		OptRef::map(
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
) -> OptRef<'static, wgpu::PipelineLayout> {
	let bind_ids = map_arr(bind_groups, |v| v.global_id());

	CompRef::into_opt_ref(
		assets.cache((&bind_ids, PreOwned(push_constants.clone())), |_| {
			gfx.device
				.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
					label: None,
					bind_group_layouts: &bind_groups,
					push_constant_ranges: &push_constants,
				})
		}),
	)
}

pub trait PipelineLayoutConfigs<K: PipelineSet>: Sized {
	fn load(
		self,
		assets: &mut AssetManager,
		gfx: &GfxContext,
	) -> OptRef<'static, PipelineLayout<K>>;
}

pub trait PipelineLayoutConfigsDefault: PipelineSet {
	fn load_default(
		assets: &mut AssetManager,
		gfx: &GfxContext,
	) -> OptRef<'static, PipelineLayout<Self>>;
}

macro_rules! impl_pipeline_layout_configs {
	($($para:ident:$field:tt),*) => {
		impl<$($para: 'static + BindGroup),*> PipelineLayoutConfigs<($($para,)*)> for ($(&$para::Config,)*) {
			fn load(self, assets: &mut AssetManager, gfx: &GfxContext) -> OptRef<'static, PipelineLayout<($($para,)*)>> {
				let bind_groups = [$(&$para::load_layout(assets, gfx, &self.$field).raw),*];
				let push_constants = [];

				let raw = load_untyped_pipeline_layout(assets, gfx, bind_groups, push_constants);

				OptRef::map(raw, PipelineLayout::wrap_ref)
			}
		}

		impl<$($para: 'static + BindGroup),*> PipelineLayoutConfigsDefault for ($($para,)*)
		where
			$($para::Config: Default),*
		{
			fn load_default(assets: &mut AssetManager, gfx: &GfxContext) -> OptRef<'static, PipelineLayout<Self>> {
				($(&<$para::Config>::default(),)*).load(assets, gfx)
			}
		}
	};
}

impl_tuples!(impl_pipeline_layout_configs);

pub trait PipelineLayoutExt: Sized {
	type Set: PipelineSet;

	fn load_default(assets: &mut AssetManager, gfx: &GfxContext) -> OptRef<'static, Self>
	where
		Self::Set: PipelineLayoutConfigsDefault;

	fn load(
		assets: &mut AssetManager,
		gfx: &GfxContext,
		configs: impl PipelineLayoutConfigs<Self::Set>,
	) -> OptRef<'static, Self>;
}

impl<T: PipelineSet> PipelineLayoutExt for PipelineLayout<T> {
	type Set = T;

	fn load_default(assets: &mut AssetManager, gfx: &GfxContext) -> OptRef<'static, Self>
	where
		Self::Set: PipelineLayoutConfigsDefault,
	{
		<Self::Set>::load_default(assets, gfx)
	}

	fn load(
		assets: &mut AssetManager,
		gfx: &GfxContext,
		configs: impl PipelineLayoutConfigs<Self::Set>,
	) -> OptRef<'static, Self> {
		configs.load(assets, gfx)
	}
}
