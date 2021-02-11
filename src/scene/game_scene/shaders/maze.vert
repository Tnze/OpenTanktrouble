#version 450 core

layout(set = 0, binding = 0) uniform Data {
    mat4 trans;
    float forecast;
} uniforms;
layout(location = 0) in vec2 v_pos;

void main() {
    gl_Position = uniforms.trans * vec4(v_pos, 0.0, 1.0);
}