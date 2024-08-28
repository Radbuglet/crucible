//#use VertexInput, Uniforms in "shared/voxel.wgsl"

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var texture: texture_2d<f32>;

@group(0) @binding(2)
var nearest_sampler: sampler;

struct VertexOutput {
	@builtin(position) clip_position: vec4f,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
	var out: VertexOutput;
	out.clip_position = uniforms.light * vec4f(in.position, 1.0);
	return out;
}
