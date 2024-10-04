#version 430 core

layout(location = 1) in vec4 input_col;
layout(location = 2) in vec3 input_normal;
layout(location = 3) in vec3 fragNormal;
layout(location = 4) in vec3 fragPosition;

layout(location = 0) out vec4 output_col;
layout(location = 1) out vec3 output_normal;

void main()
{
    vec3 lightPos = vec3(0.0, 60.0, 40.0);
    vec3 lightColor = vec3(1.0, 1.0, 1.0);

    vec3 normalizedNormal = normalize(fragNormal);

    vec3 lightDir = normalize(lightPos - fragPosition);
    float lambertian = max(0.0, dot(normalizedNormal, lightDir));

    vec3 diffuse = lambertian * lightColor;
    output_col = vec4(diffuse, 1.0) * input_col;
}
