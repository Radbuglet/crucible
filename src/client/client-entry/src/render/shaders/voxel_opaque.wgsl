// Settings
const DBM_NONE: i32 = 1;
const DBM_SHOW_LUXEL_GRID: i32 = 2;

const DEBUG_MODE: i32 = DBM_NONE;

// Uniforms
struct Uniforms {
    camera: mat4x4f,
    light: mat4x4f,
}

struct PerChunkUniforms {
    offset: vec3f,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var texture: texture_2d<f32>;

@group(0) @binding(2)
var nearest_sampler: sampler;

@group(1) @binding(0)
var light_map: texture_2d<f32>;

@group(2) @binding(0)
var<uniform> uniforms_pc: PerChunkUniforms;

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
    let position = in.position + uniforms_pc.offset;

	out.clip_position = uniforms.camera * vec4f(position, 1.0);
    out.light_space = uniforms.light * vec4f(position, 1.0);
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
    var my_lit_depth: f32 = light_space.z - 0.0001;

    // Determine whether this pixel is lit
    var is_lit = my_lit_depth < max_lit_depth;

    // Process shading mode
    if (DEBUG_MODE == DBM_SHOW_LUXEL_GRID) {
        // Determine pixel position in light-space
        var luxel = light_space.xy * vec2f(0.5, -0.5) + 0.5;
        var luxel_discrete = floor(luxel * vec2f(textureDimensions(light_map)));

        if ((luxel_discrete.x + luxel_discrete.y) % 2. == 0. || !is_lit) {
            return albedo / 2.;
        } else {
            return albedo;
        }
    } else {
        if is_lit {
            return albedo;
        } else {
            return albedo / 2.;
        }
    }
}
