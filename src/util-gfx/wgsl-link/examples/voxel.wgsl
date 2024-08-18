struct Uniforms {
    camera: mat4x4f,
    light: mat4x4f,
    light_dir: vec3f,
}

struct VoxelVertex {
    pos: vec3f,
    uv: vec2f,
    light: f32,
}

struct CompressedVertex {
    @location(0) position: vec3f,
    @location(1) uv: vec2f,
    @location(2) color: f32,
}
