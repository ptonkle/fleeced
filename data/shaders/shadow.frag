#version 300 es
precision highp float;
// A soft shadow on the ground: the mesh's first color channel is how much,
// and the sky says how strong shadows are right now.
in vec3 vColor;
uniform float uShade;
out vec4 outColor;
void main() {
  outColor = vec4(0.0, 0.0, 0.0, vColor.r * uShade);
}
