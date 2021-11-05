#version 450

//> Varyings
layout(location = 0) flat in uint v_mat;  // "flat" means that the rasterizer will not interpolate the varying.

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
