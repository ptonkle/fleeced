// The hillside's grass and the list of shaders its pages load. gl.js draws the
// world and the flock. Grass has its own program and its own packed vertices,
// so it draws straight after: first the ground's own shading, multiplied over
// what's drawn, then the blades, depth tested. Shared by the hillside page and
// the match and the menus (hillside.js, play.js, backdrop.js).

function compile(gl, type, source) {
  const shader = gl.createShader(type);
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(shader));
  return shader;
}

// A program, with a setter for the view's values and the grass's.
function program(gl, N, names, vertexSource, fragmentSource) {
  const p = gl.createProgram();
  gl.attachShader(p, compile(gl, gl.VERTEX_SHADER, vertexSource));
  gl.attachShader(p, compile(gl, gl.FRAGMENT_SHADER, fragmentSource));
  names.forEach((name, at) => gl.bindAttribLocation(p, at, name));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
  const located = (table) =>
    table
      .map(([name, size, at, call]) => ({ loc: gl.getUniformLocation(p, name), size, at, call }))
      .filter((u) => u.loc);
  const viewUniforms = located(N.uniforms);
  const grassUniforms = located(N.grass.uniforms);
  const uVP = gl.getUniformLocation(p, "uVP");
  return {
    p,
    uFade: gl.getUniformLocation(p, "uFade"),
    uSink: gl.getUniformLocation(p, "uSink"),
    use(view, values) {
      gl.useProgram(p);
      gl.uniformMatrix4fv(uVP, false, view.subarray(...N.matrix));
      viewUniforms.forEach((u) => gl[u.call](u.loc, view.subarray(u.at, u.at + u.size)));
      grassUniforms.forEach((u) => gl[u.call](u.loc, values.subarray(u.at, u.at + u.size)));
    },
  };
}

function bindAttributes(gl, attributes, stride, first) {
  attributes.forEach(([, size, type, normalized, offset], at) => {
    gl.vertexAttribPointer(at, size, gl[type], normalized, stride, first + offset);
  });
}

// `hill` hands over the baked grass and the ground. The answer draws one
// frame's grass, and its `free` lets the buffers go at the end of a match.
export function makeGrass(gl, N, shaders, hill) {
  const G = N.grass;
  const names = (list) => list.map(([name]) => name);
  const blades = program(gl, N, names(G.attributes), shaders.grassVertex, shaders.grass);
  const ground = program(gl, N, names(G.ground), shaders.groundVertex, shaders.groundShade);

  const bladeVao = gl.createVertexArray();
  gl.bindVertexArray(bladeVao);
  const bladeBuffer = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, bladeBuffer);
  gl.bufferData(gl.ARRAY_BUFFER, hill.grassVertices(), gl.STATIC_DRAW);
  const bladeIndices = gl.createBuffer();
  gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, bladeIndices);
  gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, hill.grassIndices(), gl.STATIC_DRAW);
  G.attributes.forEach((_, at) => gl.enableVertexAttribArray(at));

  const groundVao = gl.createVertexArray();
  gl.bindVertexArray(groundVao);
  const groundPoints = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, groundPoints);
  gl.bufferData(gl.ARRAY_BUFFER, hill.groundPoints(), gl.STATIC_DRAW);
  const groundIndexBuffer = gl.createBuffer();
  gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, groundIndexBuffer);
  const groundIndices = hill.groundIndices();
  gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, groundIndices, gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0);
  bindAttributes(gl, G.ground, 0, 0);
  gl.bindVertexArray(null);

  const draw = (view, values, rows) => {
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.DST_COLOR, gl.SRC_COLOR);
    gl.enable(gl.POLYGON_OFFSET_FILL);
    gl.depthMask(false);
    gl.depthFunc(gl.LEQUAL);
    ground.use(view, values);
    gl.bindVertexArray(groundVao);
    gl.drawElements(gl.TRIANGLES, groundIndices.length, gl.UNSIGNED_INT, 0);
    gl.disable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    gl.disable(gl.POLYGON_OFFSET_FILL);
    gl.depthMask(true);
    gl.depthFunc(gl.LESS);

    blades.use(view, values);
    gl.bindVertexArray(bladeVao);
    gl.bindBuffer(gl.ARRAY_BUFFER, bladeBuffer);
    let tier = null;
    for (let at = 0; at < rows.length; at += G.draw) {
      const [t, firstByte, count, firstIndex] = rows.subarray(at, at + G.draw);
      if (t !== tier) {
        tier = t;
        const [fadeFrom, fadeTo, sinkFrom, sinkTo] = G.tiers[t];
        gl.uniform4fv(blades.uFade, values.subarray(fadeFrom, fadeTo));
        gl.uniform1fv(blades.uSink, values.subarray(sinkFrom, sinkTo));
      }
      bindAttributes(gl, G.attributes, G.stride, firstByte);
      gl.drawElements(gl.TRIANGLES, count, gl.UNSIGNED_SHORT, firstIndex);
    }
    gl.bindVertexArray(null);
  };
  draw.free = () => {
    [bladeBuffer, bladeIndices, groundPoints, groundIndexBuffer].forEach((b) => gl.deleteBuffer(b));
    [bladeVao, groundVao].forEach((v) => gl.deleteVertexArray(v));
    [blades, ground].forEach((g) => gl.deleteProgram(g.p));
  };
  return draw;
}

async function source(path) {
  return (await fetch(path)).text();
}

export async function loadShaders() {
  return {
    scene: await source("data/shaders/scene.vert"),
    skin: await source("data/shaders/skin.vert"),
    lit: await source("data/shaders/scene.frag"),
    outline: await source("data/shaders/outline.frag"),
    shadow: await source("data/shaders/shadow.frag"),
    skyVertex: await source("data/shaders/sky.vert"),
    sky: await source("data/shaders/sky.frag"),
    grassVertex: await source("data/shaders/grass.vert"),
    grass: await source("data/shaders/grass.frag"),
    groundVertex: await source("data/shaders/grass.ground.vert"),
    groundShade: await source("data/shaders/grass.ground.frag"),
  };
}
