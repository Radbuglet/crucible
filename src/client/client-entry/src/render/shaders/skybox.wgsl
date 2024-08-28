//#use PI in "utils/constants.wgsl"

@group(0) @binding(0)
var<uniform> inv_proj_and_view: mat4x4<f32>;

@group(0) @binding(1)
var panorama_tex: texture_2d<f32>;

@group(0) @binding(2)
var panorama_sampler: sampler;

struct VertexOutput {
	@builtin(position) clip_position: vec4<f32>,
	@location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
	var clip_position: vec2<f32>;

	switch in_vertex_index {
		// Triangle 1
		case 0u { clip_position = vec2<f32>(-1.0, -1.0); }
		case 1u { clip_position = vec2<f32>(1.0, -1.0); }
		case 2u { clip_position = vec2<f32>(-1.0, 1.0); }
		// Triangle 2
		case 3u { clip_position = vec2<f32>(1.0, 1.0); }
		case 4u { clip_position = vec2<f32>(-1.0, 1.0); }
		default { clip_position = vec2<f32>(1.0, -1.0); }
	};

	var out: VertexOutput;
	out.clip_position = vec4<f32>(clip_position, 1.0, 1.0);
	out.uv = clip_position;

	return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	// The position of the fragment in NDC
	let ndc_pos = vec4<f32>(in.uv, 1.0, 1.0);

	// The position of the fragment in view-space
	var view_pos = inv_proj_and_view * ndc_pos;
	view_pos /= view_pos.w;

	// The position of the fragment projected down to the unit sphere.
	let view_pos_norm = normalize(view_pos);

	// The latitude of the fragment position on the normalized skybox sphere.
	// Ranges from `-PI/2` to `PI/2`.
    let latitude = PI / 2.0 - acos(view_pos_norm.y);

	// The longitude of the fragment position on the normalized skybox sphere.
	// By default, `arctan2` ranges from `-PI` to `PI`. We normalize this to `0` to `tau`.
	let longitude = atan2(view_pos_norm.z, view_pos_norm.x) + PI;

	// The equirectangular projection of this point.
	let eqp = vec2<f32>(
		longitude / (2.0 * PI),
		(latitude + PI / 2.0) / PI,
	);

	return textureSample(panorama_tex, panorama_sampler, vec2<f32>(eqp.x, 1.0 - eqp.y));
}
