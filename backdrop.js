// The hillside behind the menus. The core builds it and says where the camera
// goes (scape/backdrop.rs); this file bakes it after the first paint, keeps
// the core told what's on screen, and draws at the core's frame cap. Where
// WebGL 2 is missing or slow the scene never starts and the tweed stays.
import { Hillside } from "./pkg/tool_shim.js?v=f22ca65f99de";
import { makeRenderer } from "./gl.js?v=f22ca65f99de";
import { loadShaders, makeGrass } from "./grass.js?v=f22ca65f99de";

const canvas = document.getElementById("view");
const reduced = window.matchMedia("(prefers-reduced-motion: reduce)");

// The latest stage as JSON, drawn when the scene is ready for it.
let stage = "";
let staged = "";
let hidden = false;
// A still scene draws only when something changed.
let stale = true;
let hill = null;
// Fits the scene to the window again, once it is running.
let refit = () => {};

const breathe = () => new Promise((done) => setTimeout(done));

function capable() {
  const probe = document.createElement("canvas").getContext("webgl2", { failIfMajorPerformanceCaveat: true });
  if (!probe) return false;
  const loser = probe.getExtension("WEBGL_lose_context");
  if (loser) loser.loseContext();
  return true;
}

function sendStage() {
  if (!hill || stage === staged) return;
  staged = stage;
  stale = true;
  hill.stage(stage);
}

// What the core drew the screen with: which panels, and who's in the lineup.
export function showScene(ui, width, height) {
  stage = JSON.stringify({ width, height, dock: ui.dock, lineup: ui.lineup, cheer: ui.cheer === true });
  const back = hidden && !ui.capture && !ui.scene;
  hidden = ui.capture || ui.scene !== "";
  // a match sized the shared canvas; a still scene owes a frame
  if (back) refit();
  document.body.dataset.scene = hill && !hidden ? "on" : "";
  sendStage();
}

export async function startScene() {
  if (!capable()) return;
  const shaders = await loadShaders();
  await breathe();
  const scene = Hillside.backdrop();
  await breathe();
  const bakeFrom = performance.now();
  scene.bake();
  const bakeMs = performance.now() - bakeFrom;
  await breathe();
  const N = JSON.parse(scene.numbers());
  // made here so the page's tweed is never seen through a soft shadow's
  // alpha; the renderer's own call gets this same context
  canvas.getContext("webgl2", { alpha: false, antialias: true, stencil: true, preserveDrawingBuffer: true, powerPreference: "high-performance" });
  const library = { vertices: scene.vertices(), indices: scene.indices() };
  const renderer = makeRenderer(canvas, N, shaders, library, scene.poses());
  if (!renderer) return;
  renderer.world(scene.worldVertices(), scene.worldIndices(), scene.worldInstances());
  const grass = makeGrass(renderer.gl, N, shaders, scene);
  await breathe();

  const fit = () => {
    const [width, height] = scene.resize(canvas.clientWidth, canvas.clientHeight, window.devicePixelRatio || 1);
    renderer.resize(width, height);
    stale = true;
  };
  fit();
  refit = fit;
  window.addEventListener("resize", fit);
  const settle = () => {
    scene.still(reduced.matches);
    stale = true;
  };
  settle();
  reduced.addEventListener("change", settle);
  canvas.addEventListener("webglcontextlost", () => {
    hill = null;
    document.body.dataset.scene = "";
  });
  hill = scene;
  sendStage();

  const build = (now) => {
    scene.advance(now);
    scene.build();
    return scene.view();
  };
  const world = (view) => renderer.frame(view, scene.instances(), scene.draws(), scene.outlines());
  const blades = (view) => grass(view, scene.grassValues(), scene.grassRows());
  const frame = (now) => {
    const view = build(now);
    world(view);
    blades(view);
  };
  // For the frame-cost check: one frame's work without waiting for the screen.
  window.hillsideFrame = { frame, build, world, blades, gl: renderer.gl, bakeMs };

  const loop = (now) => {
    if (!hill) return;
    requestAnimationFrame(loop);
    if (hidden || !scene.due(now)) return;
    stale = false;
    frame(now);
    if (document.body.dataset.scene !== "on") document.body.dataset.scene = "on";
  };
  requestAnimationFrame(loop);
}
