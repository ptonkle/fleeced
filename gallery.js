// The model gallery: every model, clip and test scene, with an orbit view.
// The core reads the URL's query and builds the frames, so a screenshot is a
// link: gallery.html?model=wolf&clip=crouch&view=top&still=1
// Drag to orbit, wheel to move in and out; ?picks=1 lists the keys.
// ?scene=hide is find the Wolf: N or Space deals a new flock, R reveals, T
// opens the hiding table, whose numbers go in the address.
import init, { Gallery } from "./pkg/tool_shim.js?v=f22ca65f99de";
import { makeRenderer } from "./gl.js?v=f22ca65f99de";

const canvas = document.getElementById("view");

function list(gallery) {
  const nav = document.getElementById("picks");
  const keys = JSON.parse(gallery.keys());
  const link = (query) => {
    const a = document.createElement("a");
    a.href = "gallery.html?picks=1&" + query;
    a.textContent = query;
    nav.append(a);
  };
  keys.models.forEach((m) => link("model=" + m));
  keys.scenes.forEach((s) => link("scene=" + s));
  nav.hidden = false;
}

// The hiding table as number boxes; a change reloads with it in the address.
function tuningPanel(gallery) {
  const panel = document.getElementById("tuning");
  const params = new URLSearchParams(location.search);
  for (const { key, value } of JSON.parse(gallery.tuning())) {
    const row = document.createElement("label");
    row.textContent = key;
    const input = document.createElement("input");
    input.type = "number";
    input.value = value;
    input.addEventListener("change", () => {
      params.set("w." + key, input.value);
      params.set("seed", gallery.seed());
      location.search = params;
    });
    row.append(input);
    panel.append(row);
  }
  return panel;
}

function listenForKeys(gallery, panel) {
  window.addEventListener("keydown", (e) => {
    if (e.target instanceof HTMLInputElement) return;
    if (e.code === "KeyT") panel.hidden = !panel.hidden;
    if (gallery.key(e.code)) {
      e.preventDefault();
      const params = new URLSearchParams(location.search);
      params.set("seed", gallery.seed());
      history.replaceState(null, "", "?" + params);
    }
  });
}

async function start() {
  await init();
  const gallery = new Gallery(location.search);
  const shaders = {
    scene: await (await fetch("data/shaders/scene.vert?v=f22ca65f99de")).text(),
    skin: await (await fetch("data/shaders/skin.vert?v=f22ca65f99de")).text(),
    lit: await (await fetch("data/shaders/scene.frag?v=f22ca65f99de")).text(),
    outline: await (await fetch("data/shaders/outline.frag?v=f22ca65f99de")).text(),
    shadow: await (await fetch("data/shaders/shadow.frag?v=f22ca65f99de")).text(),
    skyVertex: await (await fetch("data/shaders/sky.vert?v=f22ca65f99de")).text(),
    sky: await (await fetch("data/shaders/sky.frag?v=f22ca65f99de")).text(),
  };
  const library = { vertices: gallery.vertices(), indices: gallery.indices() };
  const renderer = makeRenderer(canvas, JSON.parse(gallery.numbers()), shaders, library, gallery.poses());
  if (!renderer) {
    document.body.dataset.ready = "no-webgl";
    return;
  }
  const fit = () => {
    const [width, height] = gallery.resize(canvas.clientWidth, canvas.clientHeight, window.devicePixelRatio || 1);
    renderer.resize(width, height);
  };
  fit();
  window.addEventListener("resize", fit);
  canvas.addEventListener("mousemove", (e) => {
    if (e.buttons === 1) gallery.drag(e.movementX, e.movementY);
  });
  canvas.addEventListener("wheel", (e) => gallery.wheel(e.deltaY), { passive: true });
  if (new URLSearchParams(location.search).has("picks")) list(gallery);
  document.getElementById("hint").textContent = gallery.hint();
  const labels = document.getElementById("labels");
  const place = () => {
    labels.replaceChildren();
    for (const { text, left, top } of JSON.parse(gallery.labels())) {
      const label = document.createElement("span");
      label.textContent = text;
      label.style.left = left + "%";
      label.style.top = top + "%";
      labels.append(label);
    }
  };
  place();
  window.addEventListener("resize", place);
  listenForKeys(gallery, tuningPanel(gallery));

  const frame = (now) => {
    gallery.advance(now);
    const panels = gallery.panels();
    for (let k = 0; k < panels; k += 1) {
      gallery.build(k);
      renderer.panel(gallery.panel_box(k));
      renderer.frame(gallery.view(), gallery.instances(), gallery.draws(), gallery.outlines());
    }
  };
  // For the frame-cost check: one frame's work without waiting for the screen.
  window.galleryFrame = { frame, gl: renderer.gl };
  const loop = (now) => {
    frame(now);
    document.body.dataset.ready = "true";
    requestAnimationFrame(loop);
  };
  requestAnimationFrame(loop);
}

start();
