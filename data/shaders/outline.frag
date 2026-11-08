#version 300 es
precision highp float;
// The outline pass: the outline's own color, unlit, through Dog mode's rows.
in vec3 vColor;
uniform vec3 uGradeR;
uniform vec3 uGradeG;
uniform vec3 uGradeB;
out vec4 outColor;
void main() {
  outColor = vec4(dot(vColor, uGradeR), dot(vColor, uGradeG), dot(vColor, uGradeB), 1.0);
}
