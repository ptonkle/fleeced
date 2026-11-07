#version 300 es
precision highp float;
// A blade takes the same soft light and haze as everything else on the hill,
// so it can't come away from the ground it grows out of.
in vec3 vWorld;
in vec3 vNormal;
in vec3 vColor;
uniform vec3 uEye;
uniform vec2 uFog;
uniform vec3 uFogColor;
uniform vec3 uSunDir;
uniform vec3 uSun;
uniform vec3 uSky;
uniform vec3 uGround;
uniform vec4 uSoft;
uniform vec3 uGradeR;
uniform vec3 uGradeG;
uniform vec3 uGradeB;
out vec4 outColor;
void main() {
  vec3 n = normalize(vNormal);
  float sun = smoothstep(0.0, 1.0, clamp(dot(n, uSunDir) * uSoft.x + uSoft.y, 0.0, 1.0));
  vec3 fill = mix(uGround, uSky, clamp(n.y * uSoft.z + uSoft.w, 0.0, 1.0));
  vec3 lit = vColor * (fill + uSun * sun);
  float haze = smoothstep(uFog.x, uFog.y, length(vWorld - uEye));
  vec3 c = mix(lit, uFogColor, haze);
  outColor = vec4(dot(c, uGradeR), dot(c, uGradeG), dot(c, uGradeB), 1.0);
}
