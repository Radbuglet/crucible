// Uniforms
@group(0) @binding(0)
var<uniform> camera: mat4x4f;

// Vertex definitions
struct VertexInput {
	@location(0) position: vec3f,
	@location(1) color: vec3f,
}

struct InstanceInput {
	@location(2) affine_x: vec3f,
	@location(3) affine_y: vec3f,
	@location(4) affine_z: vec3f,
	@location(5) translation: vec3f,
}

struct VertexOutput {
	@builtin(position) clip_position: vec4f,
	@location(0) color: vec3f,
}

// Entry points
@vertex
fn vs_main(vert: VertexInput, inst: InstanceInput) -> VertexOutput {
	var out: VertexOutput;
	var model = mat4x4f(
		vec4f(inst.affine_x, 0.0),
		vec4f(inst.affine_y, 0.0),
		vec4f(inst.affine_z, 0.0),
		vec4f(inst.translation, 1.0),
	);

	out.clip_position = camera * model * vec4f(vert.position, 1.0);
	out.color = vert.color;

	return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
	return vec4f(in.color, 1.0);
}
