#version 300 es
// One vertex of an animal. Its bone's matrix comes from the pose table, a
// float texture: each row is one moment of one clip, three texels a bone
// holding the matrix's rows. iPose is two places in the table and how far to
// blend from the first to the second; a place's fraction blends to the next
// row, so phases run smoothly between the table's moments.
//
// The same program draws the outline: with uOutline on, each corner moves
// out along its hull normal by iOutline.a pixels, whatever the distance, and
// the fragment takes the outline's color.
in vec3 aPos;
in vec3 aNormal;
in vec3 aHull;
in vec3 aColor;
in vec3 aPaint;
in float aBone;
in vec3 iC0;
in vec3 iC1;
in vec3 iC2;
in vec3 iT;
in vec3 iPose;
in vec3 iCoat;
in vec3 iSkin;
in vec4 iOutline;
uniform mat4 uVP;
uniform highp sampler2D uPoses;
uniform vec2 uPixel;
uniform float uOutline;
out vec3 vWorld;
out vec3 vNormal;
out vec3 vColor;

vec4 texel(int x, float place) {
  int y = int(place);
  vec4 a = texelFetch(uPoses, ivec2(x, y), 0);
  vec4 b = texelFetch(uPoses, ivec2(x, y + 1), 0);
  return mix(a, b, fract(place));
}

vec4 posed(int x) {
  return mix(texel(x, iPose.x), texel(x, iPose.y), iPose.z);
}

vec3 placed(vec3 v) {
  return iC0 * v.x + iC1 * v.y + iC2 * v.z;
}

void main() {
  int bone = int(aBone);
  vec4 r0 = posed(bone);
  vec4 r1 = posed(bone + 1);
  vec4 r2 = posed(bone + 1 + 1);
  vec4 p = vec4(aPos, 1.0);
  vWorld = placed(vec3(dot(r0, p), dot(r1, p), dot(r2, p))) + iT;
  // the bone's columns in the world, for its normals
  vec3 m0 = placed(vec3(r0.x, r1.x, r2.x));
  vec3 m1 = placed(vec3(r0.y, r1.y, r2.y));
  vec3 m2 = placed(vec3(r0.z, r1.z, r2.z));
  vec3 c0 = cross(m1, m2);
  vec3 c1 = cross(m2, m0);
  vec3 c2 = cross(m0, m1);
  vNormal = aNormal.x * c0 + aNormal.y * c1 + aNormal.z * c2;
  vec4 clip = uVP * vec4(vWorld, 1.0);
  if (uOutline > 0.0) {
    vec3 hull = aHull.x * c0 + aHull.y * c1 + aHull.z * c2;
    vec2 away = (uVP * vec4(hull, 0.0)).xy / uPixel;
    float far = length(away);
    if (far > 0.0) clip.xy += away / far * iOutline.a * uPixel * clip.w;
    vColor = iOutline.rgb;
  } else {
    vColor = aColor * (aPaint.x * iCoat + aPaint.y * iSkin + aPaint.z);
  }
  gl_Position = clip;
}
