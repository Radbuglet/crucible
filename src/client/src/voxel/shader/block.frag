#version 450

//> Varyings
layout(location = 0) in vec2 v_tex_uv;

//> Uniforms
layout(set = 1, binding = 0) uniform texture2D u_texture_atlas;
layout(set = 1, binding = 1) uniform sampler u_texture_atlas_sampler;

//> Attachment outputs
layout(location = 0) out vec4 a_color;

//> Main
void main() {
    a_color = texture(sampler2D(u_texture_atlas, u_texture_atlas_sampler), v_tex_uv);
}
