#version 300 es
precision highp float;
// The ground's own shading, multiplied over what's drawn: the wind's sheen
// and shadow running across the turf, two crossing waves of darker troughs
// that each fade out as they near the size of a pixel, and a mottling of
// darker and strawier patches so the sward is never one smooth sheet. The
// output is doubled by the blend, so uHalf leaves the ground as it was.
in vec3 vWorld;
uniform vec3 uEye;
uniform vec2 uFog;
uniform vec4 uKx;
uniform vec4 uKz;
uniform vec4 uPhase;
uniform vec4 uShare;
uniform vec4 uRidge;
uniform vec3 uGroundShade;
uniform float uHalf;
uniform vec4 uMottleHash;
uniform float uMottleTop;
uniform vec3 uMottleMix;
uniform vec2 uMottleShift;
uniform vec2 uMottleBand;
uniform vec3 uMottleSizes;
uniform float uMottleFar;
uniform vec3 uMottleLow;
uniform vec3 uMottleHigh;
out vec4 outColor;
float wave(float at) {
  return (sin(at) + 1.0) * uHalf * (1.0 - clamp(fwidth(at), 0.0, 1.0));
}
// Integers: the usual fract(sin(...)) changes with a float's last bit, and a
// lattice corner got two values, a hard line down each column of cells. The
// same sums run in `scape/noise.rs`, which bakes this mottling into the blades.
float hash(vec2 c) {
  uvec2 q = uvec2(ivec2(c)) * uvec2(uMottleHash.xy);
  uint n = q.x ^ q.y;
  return float((n * n * uint(uMottleHash.z)) >> uint(uMottleHash.w)) / uMottleTop;
}
float blotch(vec2 p) {
  vec2 c = floor(p);
  vec2 f = fract(p);
  vec2 u = smoothstep(vec2(0.0), vec2(1.0), f);
  float low = mix(hash(c), hash(c + vec2(1.0, 0.0)), u.x);
  float high = mix(hash(c + vec2(0.0, 1.0)), hash(c + vec2(1.0, 1.0)), u.x);
  return mix(low, high, u.y);
}
// Each size settles to its middle value with distance, before it gets finer
// than a pixel.
float settled(vec2 p, float at) {
  return at < 1.0 ? mix(blotch(p), uHalf, at) : uHalf;
}
float mottle(vec2 p, float far) {
  float speck = settled(p * uMottleSizes.z, far * uMottleSizes.z);
  float clump = settled(p * uMottleSizes.y + uMottleShift, far * uMottleSizes.y);
  return dot(uMottleMix, vec3(blotch(p * uMottleSizes.x), clump, speck));
}
void main() {
  vec2 p = vWorld.xz;
  float gust = dot(uShare, sin(uKx * p.x + uKz * p.y - uPhase));
  float trough = (wave(dot(uRidge.xy, p)) + wave(dot(uRidge.zw, p))) * uHalf;
  float shade = 1.0 + uGroundShade.x * max(gust, 0.0) + uGroundShade.y * min(gust, 0.0) - uGroundShade.z * trough;
  float haze = smoothstep(uFog.x, uFog.y, length(vWorld - uEye));
  float m = (smoothstep(uMottleBand.x, uMottleBand.y, mottle(p, length(vWorld - uEye) * uMottleFar)) - uHalf) / uHalf;
  vec3 tone = mix(vec3(1.0), m < 0.0 ? uMottleLow : uMottleHigh, abs(m));
  outColor = vec4(mix(shade * tone, vec3(1.0), haze) * uHalf, 1.0);
}
