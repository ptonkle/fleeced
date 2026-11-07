// The WebGL 2 renderer. It uploads what the core returns and draws it; every
// size, stride, offset and uniform slot comes from the core's numbers(), so
// this file holds no layout of its own. (The machinery follows Ape Race's
// and Honey Hole's gl.js; the look is the core's.)
//
// Once: the library of meshes and the pose table, a float texture. Each
// frame, in order: the core's draws (one instanced draw per mesh, then the
// soft shadows blended on the ground); the sky, on the far plane, only where
// nothing was drawn; and the outlines. An outline is two steps: each outlined
// thing marks its whole silhouette in the stencil, wherever it stands, then
// its hull, pushed out a few pixels, is drawn only outside that mark. So an
// outline is a clean ring, and with depth off it shows through walls.

function compile(gl, type, source) {
  const shader = gl.createShader(type);
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    throw new Error(gl.getShaderInfoLog(shader));
  }
  return shader;
}

// A program and a setter for the view values it reads. Attributes share one
// numbering across programs, so one vertex array serves them all.
function program(gl, N, names, vertexSource, fragmentSource) {
  const p = gl.createProgram();
  gl.attachShader(p, compile(gl, gl.VERTEX_SHADER, vertexSource));
  gl.attachShader(p, compile(gl, gl.FRAGMENT_SHADER, fragmentSource));
  names.forEach((name, at) => gl.bindAttribLocation(p, at, name));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) {
    throw new Error(gl.getProgramInfoLog(p));
  }
  const uVP = gl.getUniformLocation(p, "uVP");
  const uniforms = N.uniforms
    .map(([name, size, at, call]) => ({ loc: gl.getUniformLocation(p, name), size, at, call }))
    .filter((u) => u.loc);
  const uOutline = gl.getUniformLocation(p, "uOutline");
  gl.useProgram(p);
  gl.uniform1i(gl.getUniformLocation(p, "uPoses"), 0);
  return {
    p,
    use(view, outline) {
      gl.useProgram(p);
      if (uVP) gl.uniformMatrix4fv(uVP, false, view.subarray(...N.matrix));
      uniforms.forEach((u) => gl[u.call](u.loc, view.subarray(u.at, u.at + u.size)));
      if (uOutline) gl.uniform1f(uOutline, outline);
    },
  };
}

// Returns null when the browser has no WebGL 2; the page then says so.
export function makeRenderer(canvas, N, shaders, library, poses) {
  const gl = canvas.getContext("webgl2", {
    antialias: true,
    stencil: true,
    preserveDrawingBuffer: true,
    powerPreference: "high-performance",
  });
  if (!gl) return null;

  const vertexNames = N.vertex.attributes.map(([name]) => name);
  const instanceNames = N.instance.attributes.map(([name]) => name);
  const names = vertexNames.concat(instanceNames);
  const cornerAt = names.length;
  names.push("aCorner");
  const build = (vertex, fragment) => program(gl, N, names, vertex, fragment);
  const rigid = build(shaders.scene, shaders.lit);
  const skinned = build(shaders.skin, shaders.lit);
  const hull = build(shaders.skin, shaders.outline);
  const decal = build(shaders.scene, shaders.shadow);
  const sky = build(shaders.skyVertex, shaders.sky);

  gl.enable(gl.DEPTH_TEST);
  gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
  gl.polygonOffset(-1, -1);

  gl.activeTexture(gl.TEXTURE0);
  gl.bindTexture(gl.TEXTURE_2D, gl.createTexture());
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA32F, N.poses.width, N.poses.height, 0, gl.RGBA, gl.FLOAT, poses);

  // One frame's instances and one set kept with the world.
  const instanceBuffers = { [N.sources.frame]: gl.createBuffer(), [N.sources.world]: gl.createBuffer() };

  function makeBank() {
    const vao = gl.createVertexArray();
    const vertices = gl.createBuffer();
    const indices = gl.createBuffer();
    gl.bindVertexArray(vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, vertices);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, indices);
    N.vertex.attributes.forEach(([, size, offset], at) => {
      gl.enableVertexAttribArray(at);
      gl.vertexAttribPointer(at, size, gl.FLOAT, false, N.vertex.stride, offset);
    });
    N.instance.attributes.forEach((_, k) => {
      const at = vertexNames.length + k;
      gl.enableVertexAttribArray(at);
      gl.vertexAttribDivisor(at, 1);
    });
    gl.bindVertexArray(null);
    return { vao, vertices, indices };
  }

  function fillBank(bank, vertexValues, indexValues) {
    gl.bindVertexArray(bank.vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, bank.vertices);
    gl.bufferData(gl.ARRAY_BUFFER, vertexValues, gl.STATIC_DRAW);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, indexValues, gl.STATIC_DRAW);
    gl.bindVertexArray(null);
  }

  const banks = { [N.banks.library]: makeBank(), [N.banks.world]: makeBank() };
  fillBank(banks[N.banks.library], library.vertices, library.indices);

  const skyVao = gl.createVertexArray();
  gl.bindVertexArray(skyVao);
  gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(N.sky), gl.STATIC_DRAW);
  gl.enableVertexAttribArray(cornerAt);
  gl.vertexAttribPointer(cornerAt, N.sky_size, gl.FLOAT, false, 0, 0);
  gl.bindVertexArray(null);

  // One row of a draw list: the mesh's index range and its instances.
  function place(row) {
    const [, bank, first, count, source, offset, many] = row;
    gl.bindVertexArray(banks[bank].vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, instanceBuffers[source]);
    N.instance.attributes.forEach(([, size, at], k) => {
      gl.vertexAttribPointer(vertexNames.length + k, size, gl.FLOAT, false, N.instance.stride, offset + at);
    });
    gl.drawElementsInstanced(gl.TRIANGLES, count, gl.UNSIGNED_INT, first, many);
  }

  function rows(list, each) {
    for (let at = 0; at < list.length; at += N.draw) each(list.subarray(at, at + N.draw));
  }

  // What each kind of draw needs of the state.
  function setPass(pass, view) {
    const flat = pass === N.passes.decal;
    gl[flat ? "enable" : "disable"](gl.BLEND);
    gl[flat ? "enable" : "disable"](gl.POLYGON_OFFSET_FILL);
    gl.depthMask(!flat);
    gl[pass === N.passes.skinned ? "enable" : "disable"](gl.CULL_FACE);
    if (pass === N.passes.rigid) rigid.use(view, 0);
    if (pass === N.passes.skinned) skinned.use(view, 0);
    if (flat) decal.use(view, 0);
  }

  function drawOutlines(view, outlines) {
    if (!outlines.length) return;
    gl.enable(gl.STENCIL_TEST);
    gl.enable(gl.CULL_FACE);
    gl.depthMask(false);
    gl.depthFunc(gl.ALWAYS);
    gl.colorMask(false, false, false, false);
    gl.stencilFunc(gl.ALWAYS, 1, 1);
    gl.stencilOp(gl.REPLACE, gl.REPLACE, gl.REPLACE);
    skinned.use(view, 0);
    rows(outlines, place);
    gl.colorMask(true, true, true, true);
    gl.stencilFunc(gl.NOTEQUAL, 1, 1);
    gl.stencilOp(gl.KEEP, gl.KEEP, gl.KEEP);
    gl.disable(gl.CULL_FACE);
    hull.use(view, 1);
    rows(outlines, (row) => {
      gl.depthFunc(row[0] === N.passes.through ? gl.ALWAYS : gl.LEQUAL);
      place(row);
    });
    gl.depthFunc(gl.LESS);
    gl.disable(gl.STENCIL_TEST);
  }

  return {
    gl,
    resize(width, height) {
      canvas.width = width;
      canvas.height = height;
      gl.viewport(0, 0, width, height);
    },
    // The static world the map builds: its meshes in the world bank, and
    // instances of them kept in the world's source.
    world(vertexValues, indexValues, instances) {
      fillBank(banks[N.banks.world], vertexValues, indexValues);
      gl.bindBuffer(gl.ARRAY_BUFFER, instanceBuffers[N.sources.world]);
      gl.bufferData(gl.ARRAY_BUFFER, instances, gl.STATIC_DRAW);
    },
    // The next frame draws into this part of the canvas only.
    panel(box) {
      const [x, y, width, height] = box;
      gl.enable(gl.SCISSOR_TEST);
      gl.viewport(x, y, width, height);
      gl.scissor(x, y, width, height);
    },
    frame(view, instances, draws, outlines) {
      const [r, g, b] = view.subarray(...N.fog_color);
      gl.clearColor(r, g, b, 1);
      gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT | gl.STENCIL_BUFFER_BIT);
      gl.bindBuffer(gl.ARRAY_BUFFER, instanceBuffers[N.sources.frame]);
      gl.bufferData(gl.ARRAY_BUFFER, instances, gl.STREAM_DRAW);
      let pass = null;
      rows(draws, (row) => {
        if (row[0] !== pass) {
          pass = row[0];
          setPass(pass, view);
        }
        place(row);
      });
      // the far plane passes where nothing nearer was drawn
      setPass(N.passes.rigid, view);
      gl.depthFunc(gl.LEQUAL);
      gl.depthMask(false);
      sky.use(view, 0);
      gl.bindVertexArray(skyVao);
      gl.drawArrays(gl.TRIANGLES, 0, N.sky.length / N.sky_size);
      gl.depthMask(true);
      gl.depthFunc(gl.LESS);
      drawOutlines(view, outlines);
      gl.depthMask(true);
      gl.bindVertexArray(null);
    },
  };
}
