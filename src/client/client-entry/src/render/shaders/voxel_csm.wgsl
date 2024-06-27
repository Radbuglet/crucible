// Uniforms
struct Uniforms {
    camera: mat4x4f,
    light: mat4x4f,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var texture: texture_2d<f32>;

@group(0) @binding(2)
var nearest_sampler: sampler;

// Vertex definitions
struct VertexInput {
	@location(0) position: vec3f,
	@location(1) uv: vec2f,
}

struct VertexOutput {
	@builtin(position) clip_position: vec4f,
}

// Entry points
@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
	var out: VertexOutput;
	out.clip_position = uniforms.light * vec4f(in.position, 1.0);
	return out;
}
