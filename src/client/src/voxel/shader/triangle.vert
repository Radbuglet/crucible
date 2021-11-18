#version 450

//> Uniforms
layout(set = 0, binding = 0) uniform CamUbo {
    mat4 proj;
} u_cam;

//> Vertex attributes
layout(location = 0) in vec3 a_pos;

//> Main
void main() {
    gl_Position = u_cam.proj * vec4(a_pos, 1.0);
}
