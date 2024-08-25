use std::{any::TypeId, borrow::Cow, marker::PhantomData, num::NonZeroU32};

use crucible_utils::{macros::impl_tuples, newtypes::transparent};
use derive_where::derive_where;

use crate::{
    buffer::BufferSlice,
    uniform::{BindGroup, BindGroupInstance, DynamicOffsetSet, PipelineLayout},
    vertex::VertexBufferLayoutSet,
    GpuStruct,
};

// === PipelineSet === //

pub trait PipelineSet: Sized + 'static {}

#[non_exhaustive]
pub struct UntypedPipelineSet;

impl PipelineSet for UntypedPipelineSet {}

impl<T: StaticPipelineSet> PipelineSet for T {}

// === StaticPipelineSet === //

pub trait StaticPipelineSet: 'static {
    fn index_of<T: 'static>() -> Option<u32>;
}

pub trait StaticPipelineSetHas<T: 'static, D>: StaticPipelineSet {
    fn index() -> u32 {
        Self::index_of::<T>().unwrap()
    }
}

macro_rules! impl_pipeline_set {
	($($para:ident:$field:tt),*) => {
		impl<$($para: 'static),*> StaticPipelineSet for ($($para,)*) {
			fn index_of<T: 'static>() -> Option<u32> {
				let index = 0;

				$(
					if TypeId::of::<T>() == TypeId::of::<$para>() {
						return Some(index);
					}

					let index = index + 1;
				)*

				let _ = index;

				None
			}
		}

		#[allow(unused_macros)]
		macro_rules! hack {
			($the_para:ident) => {
				impl<$($para: 'static),*> StaticPipelineSetHas<$the_para, disambiguators::$the_para> for ($($para,)*) {}
			}
		}

		$(hack!($para);)*
	};
}

#[allow(dead_code)]
mod disambiguators {
    use super::*;

    macro_rules! make_disambiguators {
		($($para:ident:$field:tt),*) => {
			$(pub struct $para;)*
		};
	}

    impl_tuples!(make_disambiguators; only_full);
}

impl_tuples!(impl_pipeline_set);

// === RenderPipelineBuilder === //

#[derive_where(Default)]
pub struct RenderPipelineBuilder<'a, U: PipelineSet, V: PipelineSet> {
    // Debug config
    label: Option<&'a str>,

    // Vertex shader config
    layout: Option<&'a PipelineLayout<U>>,
    _vertex_layout: PhantomData<fn(V) -> V>,
    vertex_shader: Option<RpbVertexShader<'a>>,
    fragment_shader: Option<RpbFragShader<'a>>,

    // Fixed function config
    primitive: wgpu::PrimitiveState,
    multisample: wgpu::MultisampleState,
    depth_stencil: Option<wgpu::DepthStencilState>,
    multiview: Option<NonZeroU32>,
}

struct RpbVertexShader<'a> {
    module: &'a wgpu::ShaderModule,
    entry: &'a str,
    buffers: Cow<'a, [wgpu::VertexBufferLayout<'a>]>,
}

struct RpbFragShader<'a> {
    module: &'a wgpu::ShaderModule,
    entry: &'a str,
    targets: Cow<'a, [Option<wgpu::ColorTargetState>]>,
}

impl<'a, U: PipelineSet, V: PipelineSet> RenderPipelineBuilder<'a, U, V> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn with_layout(mut self, set: &'a PipelineLayout<U>) -> Self {
        self.layout = Some(set);
        self
    }

    pub fn with_vertex_shader(
        mut self,
        module: &'a wgpu::ShaderModule,
        entry: &'a str,
        buffers: &'a impl VertexBufferLayoutSet<V>,
    ) -> Self
    where
        V: StaticPipelineSet,
    {
        self.vertex_shader = Some(RpbVertexShader {
            module,
            entry,
            buffers: buffers.layouts(),
        });
        self
    }

    pub fn with_fragment_shader(
        self,
        module: &'a wgpu::ShaderModule,
        entry: &'a str,
        format: wgpu::TextureFormat,
    ) -> Self {
        self.with_fragment_shader_custom(module, entry, format, None, wgpu::ColorWrites::all())
    }

    pub fn with_fragment_shader_alpha_blend(
        self,
        module: &'a wgpu::ShaderModule,
        entry: &'a str,
        format: wgpu::TextureFormat,
    ) -> Self {
        self.with_fragment_shader_custom(
            module,
            entry,
            format,
            Some(wgpu::BlendState::ALPHA_BLENDING),
            wgpu::ColorWrites::all(),
        )
    }

    pub fn with_fragment_shader_custom(
        self,
        module: &'a wgpu::ShaderModule,
        entry: &'a str,
        format: wgpu::TextureFormat,
        blend: Option<wgpu::BlendState>,
        write_mask: wgpu::ColorWrites,
    ) -> Self {
        self.with_multi_fragment_shader(
            module,
            entry,
            vec![Some(wgpu::ColorTargetState {
                format,
                blend,
                write_mask,
            })],
        )
    }

    pub fn with_multi_fragment_shader(
        mut self,
        module: &'a wgpu::ShaderModule,
        entry: &'a str,
        targets: impl Into<Cow<'a, [Option<wgpu::ColorTargetState>]>>,
    ) -> Self {
        self.fragment_shader = Some(RpbFragShader {
            module,
            entry,
            targets: targets.into(),
        });
        self
    }

    pub fn with_vertex_topology(mut self, topology: wgpu::PrimitiveTopology) -> Self {
        self.primitive.topology = topology;
        self
    }

    pub fn with_index_strip_topology(mut self, strip_index_format: wgpu::IndexFormat) -> Self {
        self.primitive.strip_index_format = Some(strip_index_format);
        self
    }

    pub fn with_cw_front_face(mut self) -> Self {
        self.primitive.front_face = wgpu::FrontFace::Cw;
        self
    }

    pub fn with_cull_mode(mut self, mode: wgpu::Face) -> Self {
        self.primitive.cull_mode = Some(mode);
        self
    }

    pub fn with_unclipped_depth(mut self) -> Self {
        self.primitive.unclipped_depth = true;
        self
    }

    pub fn with_line_draw_mode(mut self) -> Self {
        self.primitive.polygon_mode = wgpu::PolygonMode::Line;
        self
    }

    pub fn with_point_draw_mode(mut self) -> Self {
        self.primitive.polygon_mode = wgpu::PolygonMode::Point;
        self
    }

    pub fn with_conservative_fill(mut self) -> Self {
        self.primitive.conservative = true;
        self
    }

    pub fn with_multisample(mut self, state: wgpu::MultisampleState) -> Self {
        self.multisample = state;
        self
    }

    pub fn with_depth(
        self,
        format: wgpu::TextureFormat,
        depth_write_enabled: bool,
        depth_compare: wgpu::CompareFunction,
    ) -> Self {
        self.with_depth_stencil(wgpu::DepthStencilState {
            format,
            depth_write_enabled,
            depth_compare,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        })
    }

    pub fn with_depth_stencil(mut self, state: wgpu::DepthStencilState) -> Self {
        self.depth_stencil = Some(state);
        self
    }

    pub fn with_multiview(mut self, count: NonZeroU32) -> Self {
        self.multiview = Some(count);
        self
    }

    pub fn finish(self, device: &wgpu::Device) -> RenderPipeline<U, V> {
        RenderPipeline::wrap(
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: self.label,
                layout: self.layout.map(|u| &u.raw),
                vertex: {
                    let vs = self
                        .vertex_shader
                        .as_ref()
                        .expect("failed to create render pipeline: no vertex shader specified");

                    wgpu::VertexState {
                        module: vs.module,
                        entry_point: vs.entry,
                        buffers: &vs.buffers,
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    }
                },
                primitive: self.primitive,
                depth_stencil: self.depth_stencil,
                multisample: self.multisample,
                fragment: self.fragment_shader.as_ref().map(|fs| wgpu::FragmentState {
                    module: fs.module,
                    entry_point: fs.entry,
                    targets: &fs.targets,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                multiview: self.multiview,
            }),
        )
    }
}

// === RenderPipeline === //

#[derive_where(Debug)]
#[transparent(raw, pub wrap)]
#[repr(transparent)]
pub struct RenderPipeline<U = UntypedPipelineSet, V = UntypedPipelineSet>
where
    U: PipelineSet,
    V: PipelineSet,
{
    pub _ty: PhantomData<fn(U, V)>,
    pub raw: wgpu::RenderPipeline,
}

impl<U, V> RenderPipeline<U, V>
where
    U: PipelineSet,
    V: PipelineSet,
{
    pub fn wrap(raw: wgpu::RenderPipeline) -> Self {
        Self {
            _ty: PhantomData,
            raw,
        }
    }
}

impl<U: PipelineSet, V: PipelineSet> RenderPipeline<U, V> {
    pub fn builder<'a>() -> RenderPipelineBuilder<'a, U, V> {
        RenderPipelineBuilder::new()
    }

    pub fn bind_pipeline<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        pass.set_pipeline(&self.raw);
    }

    pub fn create_bind_group<L, D>(
        &self,
        f: impl FnOnce(&wgpu::BindGroupLayout) -> BindGroupInstance<L>,
    ) -> BindGroupInstance<L>
    where
        L: 'static + BindGroup,
        U: StaticPipelineSetHas<L, D>,
    {
        f(&self.raw.get_bind_group_layout(U::index()))
    }

    pub fn bind_group_static<'a, L, D>(
        pass: &mut wgpu::RenderPass<'a>,
        group: &'a BindGroupInstance<L>,
        offsets: &L::DynamicOffsets,
    ) where
        L: 'static + BindGroup,
        U: StaticPipelineSetHas<L, D>,
    {
        pass.set_bind_group(U::index(), &group.raw, offsets.as_offset_set().as_ref());
    }

    pub fn bind_vertex_buffer_static<'a, T, D>(
        pass: &mut wgpu::RenderPass<'a>,
        buffer: BufferSlice<'a, T>,
    ) where
        T: 'static + GpuStruct,
        V: StaticPipelineSetHas<T, D>,
    {
        pass.set_vertex_buffer(V::index(), buffer.raw);
    }

    pub fn bind_group<'a, L, D>(
        &self,
        pass: &mut wgpu::RenderPass<'a>,
        group: &'a BindGroupInstance<L>,
        offsets: &L::DynamicOffsets,
    ) where
        L: 'static + BindGroup,
        U: StaticPipelineSetHas<L, D>,
    {
        Self::bind_group_static(pass, group, offsets)
    }

    pub fn bind_vertex_buffer<'a, T, D>(
        &self,
        pass: &mut wgpu::RenderPass<'a>,
        buffer: BufferSlice<'a, T>,
    ) where
        T: 'static + GpuStruct,
        V: StaticPipelineSetHas<T, D>,
    {
        Self::bind_vertex_buffer_static(pass, buffer);
    }
}
