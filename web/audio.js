// The synth: plays what the core says to play, with Web Audio, and decides
// nothing. There are no audio files.
//
// The core (crates/core/src/sound.rs) compiles every sound into layers: a
// wave, schedules for gain, frequency and detune, filters, a slow wobble in
// pitch and a throb in level, all times in seconds from the start, every
// value final. A placed sound also comes with a level, a dullness, a position
// relative to the listener, who stays at the origin facing -z, and the point
// in the noise it starts from.
// data/sound.json holds every number; this file holds none.
//
//   startAudio(setup, noiseBytes)   call once; sound starts on the first press
//   setMasterGain(gain)             the one main volume, 0 to 1 (core: master_gain)
//   playFrame(frame)                what the core's Mixer says this frame
//   playCue(cue)  playTune(tune)  stopTune()
//
// tools/sound/render.py replays the same schedules offline: keep build() and
// that file in step.

let setup = null;
let noiseBytes = null;
let ctx = null;
let master = null;
let limiter = null;
let noise = null;
let gain = 1;
let tune = null;
let waiting = [];
const voices = new Map();
const moving = new Map();

function mark(name, value) {
  document.body.dataset[name] = value;
}

// ---- building blocks ----

// A parameter's schedule: { m: the AudioParam method, t: seconds, v, tc }.
function schedule(param, points, from) {
  (points || []).forEach((p) => param[p.m](p.v, from + p.t, p.tc));
}

function source(context, buffer, layer) {
  if (layer.wave === "noise") {
    const s = context.createBufferSource();
    s.buffer = buffer;
    s.loop = true;
    return s;
  }
  const o = context.createOscillator();
  if (layer.wave === "periodic") {
    const real = Float32Array.from(layer.periodic.real);
    const imag = Float32Array.from(layer.periodic.imag);
    o.setPeriodicWave(context.createPeriodicWave(real, imag, { disableNormalization: true }));
  } else {
    o.type = layer.wave;
  }
  o.frequency.value = 0;
  o.detune.value = 0;
  return o;
}

// A slow sine added to a parameter.
function lfo(context, rate, depth, param) {
  const o = context.createOscillator();
  const d = context.createGain();
  o.frequency.value = rate;
  d.gain.value = depth;
  o.connect(d);
  d.connect(param);
  return o;
}

// Builds one program's layers into `out`, starting at `now`. Returns what it
// started, to stop or fade early. Exported for tools/sound/page_check.py,
// which renders a program offline through this same code.
export function build(context, buffer, program, out, now) {
  return program.layers.map((layer) => {
    const src = source(context, buffer, layer);
    const start = now + layer.start;
    const stop = now + layer.stop;
    const parts = [src];
    let tail = src;
    (layer.filters || []).forEach((f) => {
      const b = context.createBiquadFilter();
      b.type = f.type;
      b.Q.value = f.q;
      b.gain.value = f.gain;
      b.frequency.value = 0;
      schedule(b.frequency, f.freq, now);
      tail.connect(b);
      tail = b;
    });
    (layer.throb || []).forEach((t) => {
      const g = context.createGain();
      g.gain.value = 1;
      parts.push(lfo(context, t.rate, t.depth, g.gain));
      tail.connect(g);
      tail = g;
    });
    const level = context.createGain();
    level.gain.value = 0;
    schedule(level.gain, layer.gain, now);
    if (src.frequency) {
      schedule(src.frequency, layer.freq, now);
      schedule(src.detune, layer.detune, now);
      (layer.wobble || []).forEach((w) => parts.push(lfo(context, w.rate, w.cents, src.detune)));
    }
    tail.connect(level);
    level.connect(out);
    parts.forEach((p) => {
      p.start(start, program.noise_at);
      if (!program.endless) p.stop(stop);
    });
    return { parts, level };
  });
}

// A program's echo off the hillside: taps of what goes into `from`.
export function echo(context, spec, from, out) {
  if (!spec) return;
  const low = context.createBiquadFilter();
  low.type = "lowpass";
  low.frequency.value = spec.filter;
  low.Q.value = spec.q;
  from.connect(low);
  spec.taps.forEach((t) => {
    const delay = context.createDelay(t.delay + 1);
    delay.delayTime.value = t.delay;
    const g = context.createGain();
    g.gain.value = t.gain;
    low.connect(delay);
    delay.connect(g);
    g.connect(out);
  });
}

// A safety net against many sounds at once. Left at its defaults this node
// is a heavy compressor and would squash the pipes.
export function makeLimiter(context, spec) {
  const c = context.createDynamicsCompressor();
  c.threshold.value = spec.threshold;
  c.knee.value = spec.knee;
  c.ratio.value = spec.ratio;
  c.attack.value = spec.attack;
  c.release.value = spec.release;
  return c;
}

export function decodeNoise(context, bytes) {
  return context.decodeAudioData(bytes.slice().buffer);
}

// ---- where a sound sits ----

function placeNodes(place) {
  const air = ctx.createBiquadFilter();
  air.type = "lowpass";
  const level = ctx.createGain();
  const panner = ctx.createPanner();
  panner.panningModel = setup.panning;
  panner.rolloffFactor = 0;
  air.connect(level);
  level.connect(panner);
  panner.connect(master);
  const nodes = { air, level, panner };
  setPlace(nodes, place, 0);
  return nodes;
}

function setPlace(nodes, place, smooth) {
  const now = ctx.currentTime;
  const glide = (param, value) => (smooth ? param.setTargetAtTime(value, now, smooth) : param.setValueAtTime(value, now));
  const [x, y, z] = place.pos;
  glide(nodes.air.frequency, place.muffle);
  glide(nodes.level.gain, place.gain);
  glide(nodes.panner.positionX, x);
  glide(nodes.panner.positionY, y);
  glide(nodes.panner.positionZ, z);
}

// ---- cues, loops and tunes ----

// Whether sound can play now. What is asked for between the first press and
// the noise being ready waits for it; otherwise the first sound is lost.
function ready(play) {
  if (ctx && !noise) waiting.push(play);
  return ctx && noise;
}

export function playCue(cue) {
  if (!ready(() => playCue(cue))) return;
  const nodes = cue.place ? placeNodes(cue.place) : null;
  build(ctx, noise, cue, nodes ? nodes.air : master, ctx.currentTime);
  if (cue.key) moving.set(cue.key, nodes);
  mark("lastSound", cue.id);
}

function startLoop(entry) {
  const nodes = placeNodes(entry.place);
  const program = setup.loops.find((l) => l.id === entry.id);
  const started = build(ctx, noise, { ...program, endless: true }, nodes.air, ctx.currentTime);
  voices.set(entry.id, { nodes, started });
}

function stopLoop(id, voice) {
  const end = ctx.currentTime + setup.fade;
  voice.nodes.level.gain.setTargetAtTime(0, ctx.currentTime, setup.smooth);
  voice.started.forEach((s) => s.parts.forEach((p) => p.stop(end)));
  voices.delete(id);
}

export function playFrame(frame) {
  if (!ctx || !noise || ctx.state !== "running") return;
  frame.cues.forEach(playCue);
  const keys = new Set(frame.moves.map((m) => m.key));
  frame.moves.forEach((m) => {
    const nodes = moving.get(m.key);
    if (nodes) setPlace(nodes, m.place, setup.smooth);
  });
  moving.forEach((nodes, key) => {
    if (!keys.has(key)) moving.delete(key);
  });
  const named = new Set(frame.loops.map((l) => l.id));
  frame.loops.forEach((l) => {
    if (!voices.has(l.id)) startLoop(l);
    else setPlace(voices.get(l.id).nodes, l.place, setup.smooth);
  });
  voices.forEach((voice, id) => {
    if (!named.has(id)) stopLoop(id, voice);
  });
  mark("audioVoices", [...voices.keys()].join(" "));
}

export function stopTune() {
  if (!tune) return;
  const end = ctx.currentTime + setup.fade;
  tune.bus.gain.setTargetAtTime(0, ctx.currentTime, setup.smooth);
  tune.started.forEach((s) => s.parts.forEach((p) => p.stop(end)));
  tune = null;
}

export function playTune(program) {
  if (!ready(() => playTune(program))) return;
  stopTune();
  const bus = ctx.createGain();
  bus.connect(master);
  echo(ctx, program.echo, bus, master);
  tune = { bus, started: build(ctx, noise, program, bus, ctx.currentTime) };
  mark("lastTune", program.id);
}

// ---- the one main volume ----

export function setMasterGain(value) {
  gain = value;
  if (master) master.gain.setTargetAtTime(value, ctx.currentTime, setup.smooth);
}

// ---- starting ----

function wake() {
  if (!ctx) {
    ctx = new AudioContext();
    master = ctx.createGain();
    master.gain.value = gain;
    limiter = makeLimiter(ctx, setup.limiter);
    master.connect(limiter);
    limiter.connect(ctx.destination);
    decodeNoise(ctx, noiseBytes).then((buffer) => {
      noise = buffer;
      mark("audio", "started");
      waiting.forEach((play) => play());
      waiting = [];
    });
  }
  if (ctx.state === "suspended") ctx.resume();
}

// Browsers start sound only after a press, so the context starts on the first.
export function startAudio(theSetup, theNoiseBytes) {
  setup = theSetup;
  noiseBytes = theNoiseBytes;
  ["pointerdown", "pointerup", "keydown"].forEach((type) => window.addEventListener(type, wake, true));
  mark("audio", "waiting");
}
