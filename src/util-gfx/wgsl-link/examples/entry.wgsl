//#use CompressedVertex in "voxel.wgsl"

struct VertexOutput {
	@builtin(position) clip_position: vec4f,
    @location(0) light_space: vec4f,
	@location(1) uv: vec2f,
    @location(2) light: f32,
    @location(3) normal: vec3f,
}

@vertex
fn vs_main(vertex: CompressedVertex) -> VertexOutput {
    var out: VertexOutput;
    return out;
}
