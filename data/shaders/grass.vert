#version 300 es
// One corner of a blade: packed, baked once a match, and bent here by the
// same four summed sines as the ground's sheen. A blade past its tier's reach
// sinks into the ground by its own distance, so no blade ever fades.
in vec4 aPos;
in vec4 aColor;
in vec4 aNormal;
uniform mat4 uVP;
uniform vec3 uEye;
uniform vec3 uOrigin;
uniform vec3 uStep;
uniform vec2 uDir;
uniform float uBend;
uniform float uTall;
uniform vec4 uKx;
uniform vec4 uKz;
uniform vec4 uPhase;
uniform vec4 uShare;
uniform vec3 uNear;
uniform vec4 uFade;
uniform float uSink;
out vec3 vWorld;
out vec3 vNormal;
out vec3 vColor;
void main() {
  vec3 w = uOrigin + aPos.xyz * uStep;
  float gust = dot(uShare, sin(uKx * w.x + uKz * w.z - uPhase));
  w.xz += uDir * uBend * aColor.a * (gust + 1.0);
  float d = length(w.xz - uEye.xz);
  float f = smoothstep(uFade.x, uFade.y, d) * (1.0 - smoothstep(uFade.z, uFade.w, d));
  w.y -= (1.0 - f) * uSink;
  vWorld = w;
  vNormal = aNormal.xyz;
  vColor = aColor.rgb * mix(uNear.z, 1.0, smoothstep(uNear.x, uNear.y, d));
  gl_Position = uVP * vec4(w, 1.0);
}
