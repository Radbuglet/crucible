struct ShaderUniformBuffer {
    proj: mat4x4<f32>,
}

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@group(1) @binding(0)
var<uniform> camera: ShaderUniformBuffer;

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    input: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera * vec4<f32>(input.position, 1.0);
    out.color = vec4<f32>(input.color, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
