#version 450 core

layout(set = 0, binding = 0) uniform Data {
    mat4 trans;
} uniforms;
layout(location = 0) in vec2 v_pos;
layout(location = 1) in vec2 i_pos;
layout(location = 2) in vec2 i_vlc;
layout(location = 3) in float i_rot;

void main() {
    mat2 rot = mat2(cos(i_rot), sin(i_rot), -sin(i_rot), cos(i_rot));
    vec2 pos = rot * v_pos + i_pos;
    gl_Position = vec4(pos, 0.0, 1.0);
}