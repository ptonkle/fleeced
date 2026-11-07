// Times one World::step in WebAssembly under node, on the worlds in
// crates/core/examples/scene/mod.rs. The budget from notes/contract.md: 100
// sheep and 8 players at most 0.4 ms a step, a 12-tick replay under 5 ms, and
// 400 sheep at most 1 ms a step.
//
//   cargo build --release --target wasm32-unknown-unknown -p tool_core --example wasm_bench
//   node scripts/bench_wasm.mjs
//
// Put the optimized figures next to these by building with
// CARGO_PROFILE_RELEASE_OPT_LEVEL=3.

import { readFile } from "node:fs/promises";

const path = process.argv[2] ?? "target/wasm32-unknown-unknown/release/examples/wasm_bench.wasm";
const bytes = await readFile(path);
const { instance } = await WebAssembly.instantiate(bytes, {});
const { load, steps, save, replay, world_bytes } = instance.exports;

const SCENES = ["calm", "driven", "cornered"];
// Long enough for the engine's optimizing tier to take over before the clock
// starts: a first load in a page sees a slower moment than these figures.
const WARM = 12000;
const TICKS = 3000;
const REPLAYS = 400;

console.log(`module ${(bytes.length / 1024).toFixed(0)} KB`);
console.log("scene       sheep  mean ms  p99 ms  worst ms  replay 12 median  worst  copy ms  KB a copy");

for (const sheep of [100, 400]) {
  SCENES.forEach((name, which) => {
    load(which, sheep);
    steps(WARM);
    const spent = new Float64Array(TICKS);
    for (let t = 0; t < TICKS; t++) {
      const start = performance.now();
      steps(1);
      spent[t] = performance.now() - start;
    }
    const sorted = [...spent].sort((a, b) => a - b);
    const mean = spent.reduce((a, b) => a + b, 0) / TICKS;

    let copy = 0;
    const replays = new Float64Array(REPLAYS);
    for (let r = 0; r < REPLAYS; r++) {
      let start = performance.now();
      save();
      copy += performance.now() - start;
      start = performance.now();
      replay(12);
      replays[r] = performance.now() - start;
    }
    const replayed = [...replays].sort((a, b) => a - b);

    console.log(
      [
        name.padEnd(10),
        String(sheep).padStart(6),
        mean.toFixed(3).padStart(8),
        sorted[Math.floor(TICKS * 0.99)].toFixed(3).padStart(7),
        sorted[TICKS - 1].toFixed(3).padStart(9),
        replayed[Math.floor(REPLAYS / 2)].toFixed(2).padStart(16),
        replayed[REPLAYS - 1].toFixed(2).padStart(6),
        (copy / REPLAYS).toFixed(4).padStart(8),
        (world_bytes() / 1024).toFixed(0).padStart(10),
      ].join(" "),
    );
  });
}
