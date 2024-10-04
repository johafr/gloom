#version 430 core

layout(location = 0) in vec3 input_pos;
layout(location = 1) in vec4 input_col;
layout(location = 2) in vec3 input_normal;

layout(location = 1) out vec4 output_col;
layout(location = 2) out vec3 output_normal;
layout(location = 3) out vec3 fragNormal;
layout(location = 4) out vec3 fragPosition;


uniform mat4 mvp_matrix;
uniform mat4 model_matrix;


void main()
{
    gl_Position = mvp_matrix * vec4(input_pos, 1.0);

    fragNormal = normalize(mat3(transpose(inverse(model_matrix))) * input_normal);
    fragPosition = vec3(model_matrix * vec4(input_pos, 1.0));

    output_col = input_col;
    output_normal = input_normal;
}