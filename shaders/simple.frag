#version 430 core

layout(location = 1) in vec4 input_col;
layout(location = 2) in vec3 input_normal;

layout(location = 0) out vec4 output_col;
layout(location = 1) out vec3 output_normal;

void main()
{
    vec3 lightDirection = normalize(vec3(0.8, -0.5, 0.6));
    float diffuse = max(0.0, dot(input_normal, -lightDirection));
    output_col = vec4(input_col.rgb * diffuse, input_col.a);
}
