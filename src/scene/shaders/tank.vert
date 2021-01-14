#version 450 core
layout(set = 0, binding = 0) uniform Data {
    mat4 trans;
} uniforms;
layout(location = 0) in vec2 position;

void main() {
    vec4 v = uniforms.trans * vec4(position, 1.0, 0.0);
    gl_Position = vec4(v[0], v[1], 0.0, v[2]);
}