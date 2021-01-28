#version 450 core

layout(location = 0) out vec4 f_color;

void main() {
    f_color = vec4(77/255.0, 77/255.0, 77/255.0, 1.0);
}