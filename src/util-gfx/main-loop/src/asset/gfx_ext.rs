use std::hash;

use crucible_utils::newtypes::impl_tuples;
use typed_wgpu::{
    pipeline::PipelineSet,
    uniform::{BindGroup, BindGroupInstance, BindGroupLayout, PipelineLayout},
};

use crate::{Asset, AssetArgs, AssetManager, GfxContext};

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

// === SamplerDesc === //

#[derive(Debug, Clone, PartialEq)]
pub struct SamplerDesc {
    pub label: Option<&'static str>,
    pub address_mode_u: wgpu::AddressMode,
    pub address_mode_v: wgpu::AddressMode,
    pub address_mode_w: wgpu::AddressMode,
    pub mag_filter: wgpu::FilterMode,
    pub min_filter: wgpu::FilterMode,
    pub mipmap_filter: wgpu::FilterMode,
    pub lod_min_clamp: f32,
    pub lod_max_clamp: f32,
    pub compare: Option<wgpu::CompareFunction>,
    pub anisotropy_clamp: u16,
    pub border_color: Option<wgpu::SamplerBorderColor>,
}

impl hash::Hash for SamplerDesc {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.label.hash(state);
        self.address_mode_u.hash(state);
        self.address_mode_v.hash(state);
        self.address_mode_w.hash(state);
        self.mag_filter.hash(state);
        self.min_filter.hash(state);
        self.mipmap_filter.hash(state);
        self.lod_min_clamp.to_bits().hash(state);
        self.lod_max_clamp.to_bits().hash(state);
        self.compare.hash(state);
        self.anisotropy_clamp.hash(state);
        self.border_color.hash(state);
    }
}

impl Eq for SamplerDesc {}

impl SamplerDesc {
    pub const NEAREST_CLAMP_EDGES: Self = Self {
        label: Some("nearest clamp edges"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Linear,
        lod_min_clamp: 0.0,
        lod_max_clamp: 0.0,
        compare: None,
        anisotropy_clamp: 1,
        border_color: None,
    };

    pub const FILTER_CLAMP_EDGES: Self = Self {
        label: Some("filter clamp edges"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        lod_min_clamp: 0.0,
        lod_max_clamp: 0.0,
        compare: None,
        anisotropy_clamp: 1,
        border_color: None,
    };
}

impl Default for SamplerDesc {
    fn default() -> Self {
        Self::NEAREST_CLAMP_EDGES
    }
}

impl SamplerDesc {
    pub fn load(&self, assets: &AssetManager, gfx: &GfxContext) -> Asset<wgpu::Sampler> {
        assets.load(gfx, (self,), |_, gfx, (desc,)| {
            gfx.device.create_sampler(&wgpu::SamplerDescriptor {
                label: desc.label,
                address_mode_u: desc.address_mode_u,
                address_mode_v: desc.address_mode_v,
                address_mode_w: desc.address_mode_w,
                mag_filter: desc.mag_filter,
                min_filter: desc.min_filter,
                mipmap_filter: desc.mipmap_filter,
                lod_min_clamp: desc.lod_min_clamp,
                lod_max_clamp: desc.lod_max_clamp,
                compare: desc.compare,
                anisotropy_clamp: desc.anisotropy_clamp,
                border_color: desc.border_color,
            })
        })
    }
}
