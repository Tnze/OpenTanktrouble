#version 450 core

layout(set = 0, binding = 0) uniform Data {
    mat4 trans;
    float forecast;
} uniforms;
layout(location = 0) in vec2 v_pos;
layout(location = 1) in vec2 i_pos;
layout(location = 2) in vec2 i_vlc;
layout(location = 3) in float i_rot;
layout(location = 4) in float i_rot_v;

void main() {
    float f_rot = i_rot + i_rot_v*uniforms.forecast;
    mat2 rot = mat2(cos(f_rot), sin(f_rot), -sin(f_rot), cos(f_rot));
    vec2 pos = rot * v_pos + (i_pos + i_vlc*uniforms.forecast);
    gl_Position = uniforms.trans * vec4(pos, 0.0, 1.0);
}