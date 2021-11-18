#version 450

//> Attachment outputs
layout(location = 0) out vec4 a_color;

//> Main
void main() {
    if (gl_FrontFacing) {
        a_color = vec4(1.0, 1.0, 0.1, 1.0);
    } else {
        a_color = vec4(1.0, 0.0, 0.0, 1.0);
    }
}
