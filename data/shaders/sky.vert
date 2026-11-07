#version 300 es
// The sky's one triangle, on the far plane; each corner carries its ray.
in vec2 aCorner;
uniform vec3 uFwd;
uniform vec3 uRight;
uniform vec3 uUp;
out vec3 vRay;
void main() {
  vRay = uFwd + uRight * aCorner.x + uUp * aCorner.y;
  gl_Position = vec4(aCorner, 1.0, 1.0);
}
