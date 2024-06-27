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

@group(1) @binding(0)
var light_map: texture_2d<f32>;

// Vertex definitions
struct VertexInput {
	@location(0) position: vec3f,
	@location(1) uv: vec2f,
}

struct VertexOutput {
	@builtin(position) clip_position: vec4f,
    @location(0) light_space: vec4f,
	@location(1) uv: vec2f,
}

// Entry points
@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
	var out: VertexOutput;
	out.clip_position = uniforms.camera * vec4f(in.position, 1.0);
    out.light_space = uniforms.light * vec4f(in.position, 1.0);
	out.uv = in.uv;
	return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    var albedo = vec4f(textureSample(texture, nearest_sampler, in.uv));

    // Determine position in light-space
    var light_space: vec3f = in.light_space.xyz;
    light_space /= in.light_space.w;

    // Determine depth in CSM
    var max_lit_depth: f32 = textureSample(
        light_map,
        nearest_sampler,
        light_space.xy * vec2f(0.5, -0.5) + 0.5,
    ).r;
    var my_lit_depth: f32 = light_space.z - 0.005;

    if my_lit_depth > 1. {
        return vec4f(0., 1., 0., 1.);
    } else if my_lit_depth < 0. {
        return vec4f(1., 0., 0., 1.);
    } else if my_lit_depth < max_lit_depth {
        return albedo;
    } else {
        return albedo / 2.;
    }
}
