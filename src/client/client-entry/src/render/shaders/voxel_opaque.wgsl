//#use VertexInput, Uniforms, PerChunkUniforms in "shared/voxel.wgsl"
//#use shadow_level in "shared/light_map.wgsl"

// Uniforms
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

// Entry points
struct VertexOutput {
	@builtin(position) clip_position: vec4f,
    @location(0) light_space: vec4f,
	@location(1) uv: vec2f,
    @location(2) light: f32,
    @location(3) normal: vec3f,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
	var out: VertexOutput;
    let position = in.position + uniforms_pc.offset;

	out.clip_position = uniforms.camera * vec4f(position, 1.0);
    out.light_space = uniforms.light * vec4f(position, 1.0);
	out.uv = in.uv;
    out.light = in.light;
    out.normal = in.normal;
	return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let albedo = vec4f(textureSample(texture, nearest_sampler, in.uv)) * in.light;
    let shadow_level = shadow_level(light_map, nearest_sampler, uniforms.light_dir, in.light_space, in.normal);

    return albedo * (1f + shadow_level) / 2f;
}
