struct Instance {
	/// The coordinate to which the `\hat x` basis vector gets mapped.
	@location(0) linear_x: vec2f,
	/// The coordinate to which the `\hat y` basis vector gets mapped.
	@location(1) linear_y: vec2f,
	/// The non-linear translational component of our affine transform.
	@location(2) translation: vec2f,
	/// The depth of the pixel.
	@location(3) depth: f32,
	/// The color of the pixel
	@location(4) color: vec4f,
}

struct Fragment {
	@builtin(position) position: vec4f,
	@location(0) color: vec4f,
}

@vertex
fn vs_main(@builtin(vertex_index) vert_index: u32, instance: Instance) -> Fragment {
	var pos: vec2<f32>;

	switch vert_index {
		// Triangle 1
		case 0u { pos = vec2<f32>(-1.0, -1.0); }
		case 1u { pos = vec2<f32>(1.0, -1.0); }
		case 2u { pos = vec2<f32>(-1.0, 1.0); }
		// Triangle 2
		case 3u { pos = vec2<f32>(1.0, 1.0); }
		case 4u { pos = vec2<f32>(-1.0, 1.0); }
		default { pos = vec2<f32>(1.0, -1.0); }
	};

	let linear = mat2x2f(instance.linear_x, instance.linear_y);
	pos = linear * pos + instance.translation;

	return Fragment(
		vec4f(pos, instance.depth, 1.0),
		instance.color
	);
}

@fragment
fn fs_main(frag: Fragment) -> @location(0) vec4f {
	return frag.color;
}
