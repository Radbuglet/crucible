#version 450

const vec3[3] s_vertices = vec3[](
    vec3(-0.2, -0.2, 0.5),
    vec3( 0.2, -0.2, 0.5),
    vec3( 0.2,  0.2, 0.5)
);

void main() {
    gl_Position = vec4(s_vertices[gl_VertexIndex], 1.0);
}
