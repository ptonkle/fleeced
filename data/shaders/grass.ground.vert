#version 300 es
// The ground again, for its own shading: where it lies, nothing else.
in vec3 aPos;
uniform mat4 uVP;
out vec3 vWorld;
void main() {
  vWorld = aPos;
  gl_Position = uVP * vec4(aPos, 1.0);
}
