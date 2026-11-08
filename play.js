// A match on the page: the mouse, the real hillside, the HUD and the sound.
// The core builds each frame and every word; this file uploads, draws and
// plays what it returns, and decides nothing.
//
// Once a match: the scene the core opens (`ui.scene` names it). Each frame:
// the look goes in before the core's pump (`readPlay`), and the frame comes
// out after it (`drawPlay`).
import { makeRenderer } from "./gl.js?v=f22ca65f99de";
import { loadShaders, makeGrass } from "./grass.js?v=f22ca65f99de";
import { makeInput } from "./input.js?v=f22ca65f99de";
import { playFrame, playTune, setMasterGain, startAudio, stopTune } from "./audio.js?v=f22ca65f99de";

const el = (id) => document.getElementById(id);
const make = (tag, cls, text) => {
  const node = document.createElement(tag);
  if (cls) node.className = cls;
  if (text !== undefined) node.textContent = text;
  return node;
};

let game = null;
let canvas = null;
let shaders = null;
let input = null;
let renderer = null;
let grass = null;
// the match whose scene is open
let open = "";
let hud = null;
let hudText = "";
let volume = null;
// how long the last scene took to open, for the frame-cost check
let openMs = 0;

// The input layer reads a table of key names, and the core's table comes with
// the first frame of play: these two are filled in then.
const controls = {};
const actions = {};

function mark(name, value) {
  const text = String(value);
  if (document.body.dataset[name] !== text) document.body.dataset[name] = text;
}

// ---- the HUD ----

function bar(cls) {
  const box = make("div", "hud-bar " + cls);
  const fill = make("div", "hud-fill");
  box.append(fill);
  return { box, fill };
}

// A word and the key that drives it, side by side.
function keyed(first, second) {
  const row = make("div", "hud-keyed");
  row.append(first, second);
  return row;
}

function buildHud() {
  const root = make("div");
  root.id = "hud";
  const top = make("div", "hud-panel hud-top");
  const counts = make("div", "hud-counts");
  const time = make("div", "hud-time");
  top.append(time, counts);

  const left = make("div", "hud-panel hud-left");
  const dial = make("div", "hud-dial");
  const dialValue = make("span", "hud-dial-value");
  dial.append(dialValue);
  const dialLabel = make("span", "hud-label");
  const dialKey = make("span", "hud-key");
  const dialBox = make("div", "hud-dial-box");
  dialBox.append(dial, keyed(dialLabel, dialKey));
  const stamina = bar("hud-stamina");
  const staminaLabel = make("span", "hud-label");
  const staminaKey = make("span", "hud-key");
  const staminaBox = make("div", "hud-stamina-box");
  staminaBox.append(keyed(staminaLabel, staminaKey), stamina.box);
  left.append(dialBox, staminaBox);

  const power = make("div", "hud-panel hud-power");
  const powerMark = make("div", "hud-mark");
  const powerIcon = make("img");
  powerMark.append(powerIcon);
  const powerName = make("div", "hud-power-name");
  const meter = bar("hud-meter");
  const powerText = make("div");
  const useKey = make("span", "hud-key");
  const useLabel = make("span", "hud-label");
  const dropKey = make("span", "hud-key");
  const dropLabel = make("span", "hud-label");
  const powerKeys = make("div", "hud-keys");
  powerKeys.append(keyed(useKey, useLabel), keyed(dropKey, dropLabel));
  powerText.append(powerName, meter.box, powerKeys);
  power.append(powerMark, powerText);

  const effects = make("div", "hud-panel hud-effects");

  const cross = make("div", "hud-cross");
  const promptKey = make("span", "hud-key");
  const promptText = make("span");
  const prompt = make("div", "hud-prompt");
  prompt.append(promptKey, promptText);
  const dig = bar("hud-dig");
  const message = make("div", "hud-message");
  const kill = make("div", "hud-kill");
  const flash = make("div", "hud-flash");
  const click = make("div", "hud-click");
  const plates = make("div", "hud-plates");
  root.append(plates, flash, top, left, power, effects, cross, prompt, dig.box, message, kill, click);
  document.body.append(root);
  return { root, cross, counts, time, dial, dialValue, dialLabel, dialKey, stamina, staminaLabel, staminaKey, power, powerIcon, powerName, meter, useKey, useLabel, dropKey, dropLabel, effects, effectRows: new Map(), prompt, promptKey, promptText, dig, message, click, kill, flash, plates, plateRows: [] };
}

function setText(node, text) {
  const shown = text || "";
  if (node.textContent !== shown) node.textContent = shown;
  node.hidden = shown === "";
}

function fill(node, percent) {
  node.style.setProperty("--fill", String(percent));
}

// A powerup at work beside its slot: its icon and how long it has left. Each
// keeps its row, so a falling bar doesn't reload its image.
function effectRow(effect) {
  let row = hud.effectRows.get(effect.name);
  if (!row) {
    const box = make("div", "hud-effect");
    const icon = make("img");
    icon.src = effect.image;
    icon.alt = effect.name;
    const meter = bar("hud-meter");
    box.append(icon, meter.box);
    row = { box, fill: meter.fill };
    hud.effectRows.set(effect.name, row);
  }
  fill(row.fill, effect.percent);
  return row.box;
}

function drawHud() {
  const text = game.hud();
  if (text === hudText) return;
  hudText = text;
  const h = JSON.parse(text).ok;
  if (!h) return;
  mark("role", h.role);
  hud.time.textContent = h.time;
  const rows = h.counts.map((c) => {
    const row = make("div", "hud-count");
    row.append(make("span", "hud-label", c.label), make("span", "hud-value", c.value));
    return row;
  });
  hud.counts.replaceChildren(...rows);
  hud.staminaLabel.textContent = h.stamina.label;
  hud.staminaKey.textContent = h.stamina.key;
  fill(hud.stamina.fill, h.stamina.percent);
  hud.dialLabel.textContent = h.dial.label;
  hud.dialKey.textContent = h.dial.key;
  hud.dialValue.textContent = h.dial.value;
  hud.dial.dataset.ready = h.dial.ready ? "on" : "";
  fill(hud.dial, h.dial.percent);
  hud.power.hidden = !h.powerup;
  if (h.powerup) {
    hud.power.dataset.icon = h.powerup.icon;
    if (hud.powerIcon.getAttribute("src") !== h.powerup.image) hud.powerIcon.src = h.powerup.image;
    hud.powerIcon.alt = h.powerup.name;
    hud.powerName.textContent = h.powerup.name;
    fill(hud.meter.fill, h.powerup.percent);
    hud.useKey.textContent = h.powerup.use.key;
    hud.useLabel.textContent = h.powerup.use.label;
    hud.dropKey.textContent = h.powerup.drop.key;
    hud.dropLabel.textContent = h.powerup.drop.label;
  }
  hud.effects.hidden = !h.effects.length;
  hud.effects.replaceChildren(...h.effects.map(effectRow));
  mark("scoped", h.scoped ? "on" : "");
  mark("zoomies", h.zoomies ? "on" : "");
  hud.cross.dataset.on = h.on_target ? "on" : "";
  hud.prompt.hidden = !h.prompt;
  hud.promptKey.textContent = h.prompt_key;
  hud.promptText.textContent = h.prompt || "";
  setText(hud.message, h.message);
  setText(hud.kill, h.kill);
  fill(hud.flash, h.flash);
  hud.dig.box.hidden = h.dig === 0;
  fill(hud.dig.fill, h.dig);
  hud.click.textContent = h.click;
}

// Nameplates over the players the core gives one, where it places them.
// Each keeps its node from frame to frame.
function drawPlates() {
  const rows = JSON.parse(game.scene_plates() || "[]");
  while (hud.plateRows.length < rows.length) {
    const node = make("div", "hud-plate");
    hud.plates.append(node);
    hud.plateRows.push(node);
  }
  hud.plateRows.forEach((node, i) => {
    const row = rows[i];
    node.hidden = !row;
    if (!row) return;
    if (node.textContent !== row.name) node.textContent = row.name;
    node.className = "hud-plate " + row.side;
    node.style.left = row.left;
    node.style.top = row.top;
  });
}

// ---- the scene ----

function fit() {
  if (!open) return;
  const [width, height] = game.scene_resize(canvas.clientWidth, canvas.clientHeight, window.devicePixelRatio || 1);
  renderer.resize(width, height);
}

function closeScene() {
  if (!open) return;
  open = "";
  game.scene_close();
  playFrame({ cues: [], moves: [], loops: [] });
  grass.free();
  grass = null;
  renderer.gl.clearColor(0, 0, 0, 0);
  renderer.gl.clear(renderer.gl.COLOR_BUFFER_BIT);
  hud.root.hidden = true;
  mark("play", "");
}

function openScene(id) {
  const from = performance.now();
  const numbers = game.scene_open();
  if (!numbers) return;
  const N = JSON.parse(numbers);
  if (!renderer) {
    const library = { vertices: game.scene_values("vertices"), indices: game.scene_rows("indices") };
    renderer = makeRenderer(canvas, N, shaders, library, game.scene_values("poses"));
    if (!renderer) {
      mark("play", "no-webgl");
      return;
    }
  }
  renderer.world(game.scene_values("world_vertices"), game.scene_rows("world_indices"), game.scene_values("world_instances"));
  grass = makeGrass(renderer.gl, N, shaders, {
    grassVertices: () => game.grass_vertices(),
    grassIndices: () => game.grass_indices(),
    groundPoints: () => game.scene_values("ground_points"),
    groundIndices: () => game.scene_rows("ground_indices"),
  });
  open = id;
  if (!hud) hud = buildHud();
  hud.root.hidden = false;
  hudText = "";
  fit();
  mark("play", "open");
  openMs = performance.now() - from;
}

const build = () => {
  game.scene_build();
  return game.scene_values("view");
};
const world = (view) => renderer.frame(view, game.scene_values("instances"), game.scene_rows("draws"), game.scene_rows("outlines"));
const blades = (view) => grass(view, game.scene_values("grass_values"), game.scene_rows("grass_rows"));

function sound() {
  const frame = JSON.parse(game.scene_sound());
  playFrame(frame);
  if (frame.tune) playTune(frame.tune);
}

// ---- what app.js calls ----

export async function startPlay(theGame) {
  game = theGame;
  canvas = el("view");
  startAudio(JSON.parse(game.sound_setup()).ok, game.sound_noise());
  input = makeInput(canvas, controls, actions, () => mark("engaged", "on"));
  document.addEventListener("pointerlockchange", () => {
    if (!input.engaged()) mark("engaged", "");
  });
  canvas.addEventListener("mousedown", () => {
    if (open && document.body.dataset.capture === "on" && !input.engaged()) input.engage();
  });
  window.addEventListener("resize", fit);
  shaders = await loadShaders();
  // For the frame-cost check: one frame's work without waiting for the screen.
  window.playFrameParts = {
    build,
    world,
    blades,
    sound,
    drawHud,
    gl: () => renderer.gl,
    openMs: () => openMs,
    game: () => game,
    look: (dx, dy) => game.look(dx, dy),
    seen: () => JSON.parse(game.seen()).ok,
  };
}

// Once a turn, before the core's pump: the mouse goes in, the buttons out.
export function readPlay() {
  if (!input) return 0;
  const { bits, dx, dy, wheel } = input.read();
  if (dx || dy) game.look(dx, dy);
  if (wheel) game.wheel(wheel);
  return bits;
}

// Once a frame, after the pump.
export function drawPlay(ui) {
  if (!shaders) return;
  if (ui.volume !== volume) {
    volume = ui.volume;
    setMasterGain(JSON.parse(game.sound_master(volume)).ok);
  }
  if (ui.scene !== open) {
    closeScene();
    if (ui.scene) openScene(ui.scene);
  }
  // the winners' tune plays on behind their screen
  if (!ui.scene && !ui.cheer) stopTune();
  if (!open) return;
  Object.keys(ui.keys || {}).forEach((code) => {
    controls[code] = code;
    actions[code] = ui.keys[code];
  });
  if (!ui.capture && input.engaged()) {
    input.release();
    mark("engaged", "");
  }
  const view = build();
  world(view);
  blades(view);
  drawHud();
  drawPlates();
  sound();
}
