#version 300 es
precision highp float;
// Soft light for a hillside under a big sky: a wrapped sun that rolls round
// a form, a sky fill from above and a ground bounce from below, and distance
// haze into the horizon. Every number comes from the core as a uniform.
// uGrade rows are the screen's red, green and blue: Dog mode's colors.
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
  // either winding: a face seen from behind is lit on the side seen
  if (!gl_FrontFacing) n = -n;
  float sun = smoothstep(0.0, 1.0, clamp(dot(n, uSunDir) * uSoft.x + uSoft.y, 0.0, 1.0));
  vec3 fill = mix(uGround, uSky, clamp(n.y * uSoft.z + uSoft.w, 0.0, 1.0));
  vec3 lit = vColor * (fill + uSun * sun);
  float haze = smoothstep(uFog.x, uFog.y, length(vWorld - uEye));
  vec3 c = mix(lit, uFogColor, haze);
  outColor = vec4(dot(c, uGradeR), dot(c, uGradeG), dot(c, uGradeB), 1.0);
}
