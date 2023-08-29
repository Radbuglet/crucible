// Uniforms
@group(0) @binding(0)
var<uniform> camera: mat4x4f;

@group(0) @binding(1)
var texture: texture_2d<f32>;

@group(0) @binding(2)
var texture_s: sampler;

// Vertex definitions
struct VertexInput {
	@location(0) position: vec3f,
	@location(1) uv: vec2f,
}

struct VertexOutput {
	@builtin(position) clip_position: vec4f,
	@location(0) uv: vec2f,
}

// Entry points
@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
	var out: VertexOutput;

	out.clip_position = camera * vec4f(in.position, 1.0);
	out.uv = in.uv;
	return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
	return vec4f(textureSample(texture, texture_s, in.uv));
}
