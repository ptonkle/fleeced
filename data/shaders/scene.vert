#version 300 es
// One vertex of something that does not bend: a wall, a tree, a Carcass, the
// ground. The instance's three columns and translation place it, and its
// paint says how much of the instance's coat and skin it takes.
in vec3 aPos;
in vec3 aNormal;
in vec3 aColor;
in vec3 aPaint;
in vec3 iC0;
in vec3 iC1;
in vec3 iC2;
in vec3 iT;
in vec3 iCoat;
in vec3 iSkin;
uniform mat4 uVP;
out vec3 vWorld;
out vec3 vNormal;
out vec3 vColor;
void main() {
  vWorld = iC0 * aPos.x + iC1 * aPos.y + iC2 * aPos.z + iT;
  // normals by the cofactor matrix, so a stretched shape shades right
  vNormal = aNormal.x * cross(iC1, iC2) + aNormal.y * cross(iC2, iC0) + aNormal.z * cross(iC0, iC1);
  vColor = aColor * (aPaint.x * iCoat + aPaint.y * iSkin + aPaint.z);
  gl_Position = uVP * vec4(vWorld, 1.0);
}
