// The sound test page: every sound and tune, a button each, drawn from the
// core's listing. It decides nothing.
import init, { SoundDesk } from "./pkg/tool_shim.js?v=f22ca65f99de";
import { startAudio, setMasterGain, playCue, playTune, stopTune, playFrame } from "./audio.js?v=f22ca65f99de";

const el = (id) => document.getElementById(id);
const make = (tag, text) => {
  const node = document.createElement(tag);
  if (text !== undefined) node.textContent = text;
  return node;
};

const quiet = { cues: [], moves: [], loops: [] };

let desk = null;
let flock = 0;

function ok(text) {
  const result = JSON.parse(text);
  if (result.error) el("message").textContent = result.error;
  return result.ok;
}

function list(heading, rows, press) {
  const box = make("section");
  box.append(make("h2", heading));
  rows.forEach((row) => {
    const button = make("button", row.label);
    button.dataset.sound = row.id;
    button.addEventListener("click", () => press(row));
    box.append(button);
  });
  return box;
}

function frame() {
  playFrame(flock ? ok(desk.flock_frame(flock)) : quiet);
  requestAnimationFrame(frame);
}

function setVolume() {
  setMasterGain(ok(desk.master(el("volume-slider").value)));
}

async function main() {
  await init();
  const seed = new Uint32Array(1);
  crypto.getRandomValues(seed);
  desk = new SoundDesk(seed[0]);
  const page = ok(desk.listing());
  startAudio(ok(desk.setup()), desk.noise());
  document.title = page.words.title;
  el("title").textContent = page.words.title;
  el("volume-label").textContent = page.words.volume;
  el("volume-slider").addEventListener("input", setVolume);
  setVolume();
  const flocks = page.flocks.map((count) => ({ id: count, label: `${count} ${page.words.sheep}`, count }));
  flocks.push({ id: "stop", label: page.words.stop, count: 0 });
  const tunes = [...page.tunes, { id: "stop-tune", label: page.words.stop }];
  const play = (c) => playCue(ok(desk.cue(c.id)));
  el("lists").replaceChildren(
    list(page.words.tunes, tunes, (t) => (t.id === "stop-tune" ? stopTune() : playTune(ok(desk.tune(t.id))))),
    list(page.words.cues, page.cues, play),
    list(page.words.loops, page.loops, play),
    list(page.words.flocks, flocks, (f) => {
      flock = f.count;
    }),
  );
  document.body.dataset.ready = "true";
  frame();
}

main();
