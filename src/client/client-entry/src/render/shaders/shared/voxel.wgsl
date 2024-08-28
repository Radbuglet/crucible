struct VertexInput {
	@location(0) position: vec3f,
	@location(1) uv: vec2f,
    @location(2) light: f32,
    @location(3) normal: vec3f,
}

struct Uniforms {
    camera: mat4x4f,
    light: mat4x4f,
    light_dir: vec3f,
}

struct PerChunkUniforms {
    offset: vec3f,
}
