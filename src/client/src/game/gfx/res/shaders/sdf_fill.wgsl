type Color = vec4<f32>;

struct VertexInput {
	@location(0) position: vec3<f32>,
	@location(1) color: Color,
}

struct VertexOutput {
	@builtin(position) clip_position: vec4<f32>,
	@location(0) color: Color,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, input: VertexInput) -> VertexOutput {
	var vertices = array<vec2<f32>, 4>(
		// 4----3  ^ +z
		// |    |  |
		// 1----2  *---> +x
		vec2<f32>(-1.0, -1.0),
		vec2<f32>(1.0, -1.0),
		vec2<f32>(1.0, 1.0),
		vec2<f32>(-1.0, 1.0),
	);

	var out: VertexOutput;
	out.clip_position = vec4<f32>(input.position, 0.0);
	out.color = input.color;
	return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) Color {
	return input.color;
}
