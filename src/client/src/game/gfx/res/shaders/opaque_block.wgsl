// Uniforms
@group(0) @binding(0)
var<uniform> camera: mat4x4<f32>;

@group(0) @binding(1)
var texture: texture_2d<f32>;

// Vertex definitions
struct VertexInput {
	@location(0) position: vec3<f32>,
	@location(1) uv: vec2<f32>,
}

struct VertexOutput {
	@builtin(position) clip_position: vec4<f32>,
	@location(0) uv: vec2<f32>,
}

// Entry points
@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
	var out: VertexOutput;

	out.clip_position = camera * vec4<f32>(in.position, 1.0);
	out.uv = in.uv;
	return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	return vec4<f32>(textureLoad(texture, vec2<i32>(in.uv), 0));
}
