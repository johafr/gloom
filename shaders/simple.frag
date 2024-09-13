#version 430 core

layout(location = 1) out vec4 output_col;
layout(location = 1) in vec4 input_col;

void main()
{
    output_col = input_col;
}