#version 450

//> Varyings
layout(location = 0) in float v_dist;

//> Attachment outputs
layout(location = 0) out vec4 a_color;

//> Main
void main() {
    a_color = vec4(218. / 255., 84. / 255., 255. / 255., 1.0) * v_dist;
}
