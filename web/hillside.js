// The hillside on its own: the map's world, a flock, the sky and the grass.
// The core reads the URL's query, so a screenshot is a link:
// hillside.html?seed=3&sheep=100&phase=0&x=120&z=60&yaw=30&tick=90
//
// gl.js draws the world and the flock, and grass.js the grass, straight after.
import init, { Hillside } from "./pkg/tool_shim.js";
import { makeRenderer } from "./gl.js";
import { loadShaders, makeGrass } from "./grass.js";

const canvas = document.getElementById("view");

async function start() {
  await init();
  const hill = new Hillside(location.search);
  const bakeFrom = performance.now();
  hill.bake();
  const bakeMs = performance.now() - bakeFrom;
  const shaders = await loadShaders();
  const N = JSON.parse(hill.numbers());
  const library = { vertices: hill.vertices(), indices: hill.indices() };
  const renderer = makeRenderer(canvas, N, shaders, library, hill.poses());
  if (!renderer) {
    document.body.dataset.ready = "no-webgl";
    return;
  }
  renderer.world(hill.worldVertices(), hill.worldIndices(), hill.worldInstances());
  const grass = makeGrass(renderer.gl, N, shaders, hill);
  const fit = () => {
    const [width, height] = hill.resize(canvas.clientWidth, canvas.clientHeight, window.devicePixelRatio || 1);
    renderer.resize(width, height);
  };
  fit();
  window.addEventListener("resize", fit);

  const build = (now) => {
    hill.advance(now);
    hill.build();
    return hill.view();
  };
  const world = (view) => renderer.frame(view, hill.instances(), hill.draws(), hill.outlines());
  const blades = (view) => grass(view, hill.grassValues(), hill.grassRows());
  const frame = (now) => {
    const view = build(now);
    world(view);
    blades(view);
  };
  // For the frame-cost check: one frame's work without waiting for the screen.
  window.hillsideFrame = { frame, build, world, blades, gl: renderer.gl, bakeMs };
  const loop = (now) => {
    frame(now);
    document.body.dataset.ready = "true";
    requestAnimationFrame(loop);
  };
  requestAnimationFrame(loop);
}

start();
