#version 430 core

layout(location = 0) in vec3 input_pos;
layout(location = 1) in vec4 input_col;

layout(location = 0) out vec3 output_pos;
layout(location = 1) out vec4 output_col;

uniform mat4 transformation_matrix;

void main()
{
    gl_Position = transformation_matrix * vec4(input_pos, 1.0);
    output_pos = input_pos;
    output_col = input_col;
}