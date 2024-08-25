use crucible_assets::{Asset, AssetArgs, AssetManager};
use crucible_utils::macros::impl_tuples;
use main_loop::GfxContext;
use typed_wgpu::{BindGroup, BindGroupInstance, BindGroupLayout, PipelineLayout, PipelineSet};

// === BindGroupExt === //

pub trait BindGroupExt: BindGroup<Config = Self::Config2> {
    type Config2: AssetArgs;

    fn load_layout_raw(
        assets: &AssetManager,
        gfx: &GfxContext,
        config: Self::Config,
    ) -> Asset<wgpu::BindGroupLayout> {
        assets.load(gfx, config, |_, gfx, config| {
            Self::create_layout(&gfx.device, &config).raw
        })
    }

    fn load_layout(
        assets: &AssetManager,
        gfx: &GfxContext,
        config: Self::Config,
    ) -> Asset<BindGroupLayout<Self>> {
        Asset::map(
            Self::load_layout_raw(assets, gfx, config),
            BindGroupLayout::wrap_ref,
        )
    }

    fn load_instance(
        &self,
        assets: &AssetManager,
        gfx: &GfxContext,
        config: Self::Config,
    ) -> BindGroupInstance<Self> {
        self.create_instance(
            &gfx.device,
            &Self::load_layout_raw(assets, gfx, config),
            &config,
        )
    }
}

impl<T: BindGroup> BindGroupExt for T
where
    T::Config: AssetArgs,
{
    type Config2 = T::Config;
}

// === PipelineLayoutExt === //

pub fn load_untyped_pipeline_layout<const N: usize, const M: usize>(
    assets: &AssetManager,
    gfx: &GfxContext,
    bind_groups: [&wgpu::BindGroupLayout; N],
    push_constants: [wgpu::PushConstantRange; M],
) -> Asset<wgpu::PipelineLayout> {
    let bind_ids = bind_groups.map(|v| v.global_id());

    assets.load(
        (gfx, bind_groups),
        (&bind_ids, &push_constants),
        |_, (gfx, bind_groups), (_, push_constants)| {
            gfx.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &bind_groups,
                    push_constant_ranges: push_constants,
                })
        },
    )
}

pub trait PipelineLayoutConfigs<K: PipelineSet>: Sized {
    fn load(self, assets: &AssetManager, gfx: &GfxContext) -> Asset<PipelineLayout<K>>;
}

pub trait PipelineLayoutConfigsDefault: PipelineSet {
    fn load_default(assets: &AssetManager, gfx: &GfxContext) -> Asset<PipelineLayout<Self>>;
}

macro_rules! impl_pipeline_layout_configs {
	($($para:ident:$field:tt),*) => {
		impl<$($para: 'static + BindGroupExt),*> PipelineLayoutConfigs<($($para,)*)> for ($($para::Config,)*) {
			fn load(self, assets: &AssetManager, gfx: &GfxContext) -> Asset<PipelineLayout<($($para,)*)>> {
				let bind_groups = [$(&$para::load_layout(assets, gfx, self.$field).raw),*];
				let push_constants = [];

                Asset::map(
                    load_untyped_pipeline_layout(assets, gfx, bind_groups, push_constants),
                    PipelineLayout::wrap_ref,
                )
			}
		}

		impl<$($para: 'static + BindGroupExt),*> PipelineLayoutConfigsDefault for ($($para,)*)
		where
			$($para::Config: Default),*
		{
			fn load_default(assets: &AssetManager, gfx: &GfxContext) -> Asset<PipelineLayout<Self>> {
				($(<$para::Config>::default(),)*).load(assets, gfx)
			}
		}
	};
}

impl_tuples!(impl_pipeline_layout_configs);

pub trait PipelineLayoutExt: Sized {
    type Set: PipelineSet;

    fn load_default(assets: &AssetManager, gfx: &GfxContext) -> Asset<Self>
    where
        Self::Set: PipelineLayoutConfigsDefault;

    fn load(
        assets: &AssetManager,
        gfx: &GfxContext,
        configs: impl PipelineLayoutConfigs<Self::Set>,
    ) -> Asset<Self>;
}

impl<T: PipelineSet> PipelineLayoutExt for PipelineLayout<T> {
    type Set = T;

    fn load_default(assets: &AssetManager, gfx: &GfxContext) -> Asset<Self>
    where
        Self::Set: PipelineLayoutConfigsDefault,
    {
        <Self::Set>::load_default(assets, gfx)
    }

    fn load(
        assets: &AssetManager,
        gfx: &GfxContext,
        configs: impl PipelineLayoutConfigs<Self::Set>,
    ) -> Asset<Self> {
        configs.load(assets, gfx)
    }
}
