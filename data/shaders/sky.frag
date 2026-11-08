#version 300 es
precision highp float;
// The sky behind everything: the horizon's haze up to the zenith, a glow
// round whatever lights the hill, widest when it's low, the sun and moon as
// soft discs, and stars at night. After the sun has gone the key light is the
// moon, so the first disc lies on the second. Dog mode's rows apply here too.
in vec3 vRay;
uniform vec3 uFogColor;
uniform vec3 uZenith;
uniform vec2 uSkyBand;
uniform vec3 uSunDir;
uniform vec2 uSunDisc;
uniform vec3 uSunColor;
uniform vec3 uMoonDir;
uniform vec2 uMoonDisc;
uniform vec3 uMoonColor;
uniform vec3 uStars;
uniform vec4 uHash;
uniform vec3 uGradeR;
uniform vec3 uGradeG;
uniform vec3 uGradeB;
out vec4 outColor;
void main() {
  vec3 d = normalize(vRay);
  vec3 c = mix(uFogColor, uZenith, smoothstep(uSkyBand.x, uSkyBand.y, d.y));
  float glow = max(dot(d, uSunDir), 0.0);
  glow *= glow;
  glow *= glow;
  glow *= glow;
  c = mix(c, uSunColor, glow * (1.0 - uSunDir.y) * uFogColor.b);
  if (uStars.z > 0.0) {
    vec3 cell = d * uStars.x;
    float h = fract(sin(dot(floor(cell), uHash.xyz)) * uHash.w);
    // a star is a point at its cell's middle, a pixel or two across
    vec3 edge = min(fract(cell), 1.0 - fract(cell));
    float wide = fwidth(cell.x) + fwidth(cell.y) + fwidth(cell.z);
    float spot = 1.0 - clamp(length(1.0 - edge - edge) / wide, 0.0, 1.0);
    c += vec3(uStars.z * smoothstep(uStars.y, 1.0, h) * spot);
  }
  c = mix(c, uSunColor, smoothstep(uSunDisc.x, uSunDisc.y, dot(d, uSunDir)));
  c = mix(c, uMoonColor, smoothstep(uMoonDisc.x, uMoonDisc.y, dot(d, uMoonDir)));
  outColor = vec4(dot(c, uGradeR), dot(c, uGradeG), dot(c, uGradeB), 1.0);
}
