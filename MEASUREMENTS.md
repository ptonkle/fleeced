# Measurements

Stage 4, recorded by the agent before building and again at handoff. `B2 Require step-by-step justification`.

The state measured rather than recalled, so later changes can be compared with it. A probe prints numbers and asserts loose bounds at most; record its output here with the command that produced it.

| What | Command | Result | When |
| --- | --- | --- | --- |
| Toolchain | `rustc --version; wasm-bindgen --version` | | |
| Core tests | `./scripts/count-tests.sh` | | |
| Full check | `./scripts/check.sh` | | |
| WebAssembly size | printed by `./scripts/build.sh` | | |
| Breaks caught | `python3 scripts/break_tests.py` | | |
| Map generation, native release profile | `cargo run --release --example map_probe -- 1000` | Mean 1.02 ms, p99 1.59 ms, worst 3.27 ms over 1000 seeds, against a limit of about 20 ms | 2026-11-06 |
| Sheep start spots | same | 0.07 ms for 100 sheep, 0.36 ms for 400 | 2026-11-06 |
| Collision, native release profile | same | `push_out` plus `height` 50 ns a body; a body pressed against a wall 59 ns | 2026-11-06 |
| Flock's route | same | Proved 5.00 m at the narrowest and 7.49 m at the widest over 300 seeds. `crates/core/tests/map_seeds.rs` holds 4.7 m at every point on 300 seeds, and each gap at 6 to 10 m | 2026-11-06 |
| Tunnels | same | Shortest 90.00 m, closest two mouths 30.50 m over 1000 seeds | 2026-11-06 |
| Steepest ground inside the walls | same | 0.22 m in 1 m over 1000 seeds | 2026-11-06 |
| Map as JSON | same | 56 KB | 2026-11-06 |
| Map breaks caught | `python3 scripts/break_tests.py map` | 42 of 42, none by a failure to compile | 2026-11-06 |

## Hillside

Chromium under Xvfb, on a real GPU by way of ANGLE, OpenGL, Mesa's D3D12 driver and WSL: "ANGLE (Microsoft Corporation, D3D12 (AMD Radeon(TM) Graphics), OpenGL 4.6)". The first number is forced through the GPU with `readPixels`. `EXT_disjoint_timer_query_webgl2` is available.

| What | Command | Result | When |
| --- | --- | --- | --- |
| Frame, 100 sheep, 1920x1080, third-person view, day | `python3 tests/browser/hillside.py dist OUT --cost perf="seed=3&sheep=100&tick=90"` | median 6.70 ms, 95th percentile 7.30 ms; GPU timer 4.61 ms | 2026-11-06 |
| The same, split | same | core 0.20 ms, world and flock 5.00 ms, grass 2.50 ms | 2026-11-06 |
| Frame, 300 sheep | same, `sheep=300` | median 8.05 ms, 95th percentile 8.70 ms; GPU timer 5.59 ms | 2026-11-06 |
| Frame with thick grass, 100 sheep, one stand before and after | `python3 tests/browser/hillside.py dist OUT --cost perf="seed=3&sheep=100&tick=90&x=208&z=62&yaw=-85"` | Quietest runs: before 7.10 ms, after 9.85 ms, GPU timer 5.20 ms to 6.57 ms. With other agents running: 12.0 ms to 12.5 ms, GPU timer 8.0 ms to 9.5 ms | 2026-11-06 |
| The same, 300 sheep | same, `sheep=300` | Quietest runs: before 9.00 ms, after 11.00 ms, GPU timer 6.36 ms to 7.86 ms | 2026-11-06 |
| Frame before the four-part attributes (R1.700) | same | 878 ms, 880 of it grass | 2026-11-06 |
| Grass bake, WebAssembly | printed by the same command | about 1,100 ms, once a match (was 385 ms) | 2026-11-06 |
| Grass baked | `the_whole_map_bakes_to_the_size_the_notes_planned` | 384,153 blades and 43,241 prisms on seed 3, 28.7 MB | 2026-11-06 |
| Grass bake, WebAssembly | printed by the same command | 385 ms, once a match | 2026-11-06 |
| Menu backdrop, 1920x1080, title / lobby of four | scratch run of `hillside.COST` on `index.html?fast&net=off&scene` | median 8.9 / 10.0 ms, GPU timer 6.0 / 7.1 ms, capped at 30 frames a second | 2026-11-06 |
| Menu backdrop startup | same | title 75 ms, scene in 1.2 s, longest block 539 ms | 2026-11-06 |
| Grass baked | `the_whole_map_bakes_to_the_size_the_notes_planned` | 138,793 prisms, 13.3 MB | 2026-11-06 |
| `World::step` in WebAssembly, 100 sheep and 8 players | `cargo build --release --target wasm32-unknown-unknown -p tool_core --example wasm_bench`, then `node scripts/bench_wasm.mjs` | Budget 0.4 ms. Mean 0.016 calm, 0.033 driven, 0.023 cornered; p99 at most 0.057; worst 0.185 ms | 2026-11-06 |
| `World::step` in WebAssembly, 400 sheep and 8 players | same | Budget 1 ms. Mean 0.069 calm, 0.090 driven, 0.089 cornered; p99 at most 0.143; worst 0.275 ms | 2026-11-06 |
| 12-tick replay in WebAssembly | same | Budget 5 ms. At 100 sheep median at most 0.41, worst 0.86 ms; at 400 sheep median at most 1.09, worst 1.60 ms | 2026-11-06 |
| The same with `opt-level = 3`, `codegen-units = 1` | `CARGO_PROFILE_RELEASE_OPT_LEVEL=3 CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1`, same build | 100 sheep mean at most 0.027 ms, 400 sheep at most 0.064 ms; replay worst 1.49 ms; module 110 KB against 82 KB. Not worth it for the sim alone | 2026-11-06 |
| A copy of the World | same | 7 KB and 0.7 us at 100 sheep, 26 KB and 1.1 us at 400 | 2026-11-06 |
| `Match::step` in WebAssembly, 100 sheep and 8 players, every button in use | `cargo build --release --target wasm32-unknown-unknown -p tool_core --example wasm_match`, then `node scripts/bench_wasm.mjs target/wasm32-unknown-unknown/release/examples/wasm_match.wasm` | Budget 0.4 ms. Two runs at load about 10: mean 0.032 to 0.052; p99 0.114 to 0.379; worst 0.459 to 6.478 ms | 2026-11-06 |
| 12-tick replay of `Match`, 100 sheep | same | Budget 5 ms. Median 0.29 to 0.55; worst 1.50 to 9.68 ms, over budget on 3 of 6 scene runs under that load (R1.1017) | 2026-11-06 |
| `Match::step` in WebAssembly, 400 sheep and 8 players, every button in use | same | Budget 1 ms. Mean 0.082 to 0.382; p99 at most 4.413; worst 17.707 ms | 2026-11-06 |
| One copy of a match, 100 sheep | same | 0.002 to 0.016 ms; 7 KB of bodies and sheep | 2026-11-06 |
| `Map::generate` and `Match::new` in WebAssembly, first load | a timed `load` under node | 5.2 ms; `Match::new` alone 0.5 ms | 2026-11-06 |
| `World::step`, native release profile | `cargo run --release -p tool_core --example bench` | 100 sheep mean 0.010 to 0.024 ms, 400 sheep 0.031 to 0.058 ms | 2026-11-06 |
| Sheep stuck on the real map | `cargo test -p tool_core --test edges -- --nocapture --test-threads=1` | 0 of 5112 sheep over 852 runs, again at the doubled speeds, seeds 1 to 4: outer corners, Fold outside corners, inner wall ends, gap wall ends, behind trees and cairns, under three ways of pressing. 0 left within 3 m of a tip after a Sheepdog goes in between. The last-resort run fired 0 times | 2026-11-06 |
| Pile-up at a gap's wall ends | same | 14 runs, 60 of 60 through each, longest pressed hold 0.1 s | 2026-11-06 |
| Full drive on the real map, 30 sheep, scripted Sheepdog | same | All folded in 53 to 100 s, seeds 1 to 6, on the 325 m by 175 m map, at the doubled speeds (110 to 167 s before) | 2026-11-06 |
| Drive into a small Fold, flat pasture | `cargo test -p tool_core --test herding -- --nocapture --test-threads=1` | 30 sheep 35 to 37 s on seeds 1 to 5 | 2026-11-06 |
| `Match::step` and a 12-tick replay in WebAssembly, 100 sheep, quiet machine | `wasm_match` as above, at load 1 to 3 | Step mean 0.017 to 0.034, p99 0.046 to 0.077, worst 0.285 to 0.359 ms. Replay median 0.19 to 0.39, worst 0.53 to 0.99 ms against 5 ms | 2026-11-06 |
| The same, 400 sheep | same | Step mean 0.069 to 0.186, worst 1.256 ms. Replay worst 4.22 ms | 2026-11-06 |
| Play frame, 100 sheep, 1920x1080, AMD integrated GPU under Xvfb | a timed Solo match on the page | Added CPU: core build 0.2 median, 0.5 worst; draw calls 0.2, 0.6; HUD and sound 0.1, 0.3; total 0.4, 1.0 ms. Pump 0.1, 0.5 ms | 2026-11-06 |
| Play frame gaps | same, against bare `hillside.html` | Play median 16.6 to 17.4 ms, p99 22 to 27 ms, 8 to 174 late frames between runs. Hillside median 16.7, p99 17.7, none late. The cause isn't HUD, menus or the core (R1.1608) | 2026-11-06 |
| Scene opening | same | 857 ms, inside the countdown | 2026-11-06 |
| Two browsers online, 100 sheep, both turning every frame | same, pump timed | Pump median 0.1, p99 0.3, worst 0.5 ms at local latency, checksums equal at tick 2790. Replays are shallow here: this does not show a replay under real lag | 2026-11-06 |
| `Match::step` in WebAssembly, 100 sheep, bots in seven of eight seats | `WASM_MATCH_BOTS=1 cargo build --release --target wasm32-unknown-unknown -p tool_core --example wasm_match`, then `node scripts/bench_wasm.mjs` as above, at load 6 to 7 | Step mean 0.018 to 0.023, p99 at most 0.051, worst 0.141 ms. Replay of 12 ticks median 0.23 to 0.28, worst 0.55 ms: under the 0.99 ms before bots, which pressed every button in every seat | 2026-11-06 |
| The same, 400 sheep | same | Step mean 0.079 to 0.114, worst 0.318 ms. Replay median 0.94 to 1.19, worst 2.43 ms | 2026-11-06 |

## Bots against bots

The lobby's defaults for each line-up, 8 minutes, powerups on, seeds 1 to 10, each seat a bot, at the doubled speeds. `cargo run --release -p tool_core --example bot_table 10`, 2026-11-06.

| Line-up | Matches | Sheepdogs win | By the clock | Wolves win | Mean length | Shortest to longest | Mean eats | Mean in the Fold | Wolves killed, per match |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 v 1 | 10 | 0 | 0 | 10 | 416 s | 410 to 422 s | 10 | 30 | 0 |
| 2 v 2 | 10 | 0 | 0 | 10 | 445 s | 416 to 476 s | 20 | 71 | 1 |
| 4 v 4 | 10 | 6 | 6 | 4 | 472 s | 455 to 480 s | 38 | 131 | 7 |
| 1 v 7 | 10 | 0 | 0 | 10 | 454 s | 422 to 472 s | 70 | 56 | 3 |
| 7 v 1 | 10 | 6 | 0 | 4 | 403 s | 283 to 476 s | 8 | 70 | 1 |

Before the speeds doubled, the same table had 18, 34, 70, 12 and 53 sheep in the Fold, and Wolves won 40 of 50.

What the table comes down to:

- The Wolf bot's own pace, `BOT_MEAL_EVERY_TICKS`, still sets the result: 10 meals at 44 s is 396 s plus the first wait. Eating doesn't get faster with speed, since the dial is flat (FL-3), so 44 s stays (R1.2006).
- Bots still fill no default Fold: a lone bot reaches 30 of its 75 in 416 s. It drives one sheep at a time, where the scripted Sheepdog of `edges` folds 30 sheep in 53 to 100 s, so this measures the bot and not the game (R1.2006).
- `BOT_JOB_TICKS` at 25 s instead of 45 s changed nothing, so it stays.
- A Wolf that bolts in time still can't be run down: a sprint gains 1.2 m/s for 6 s, or 7.2 m, against `BOT_FLEE_REACH` of 4 m and a decision every 0.4 s, in which a Sheepdog covers 6.4 m. So bots kill more now, 7 a match at 4 v 4.

Lobbies tried, 10 seeds, 8 minutes. `BOT_TABLE_SHEEP`, `BOT_TABLE_FOLD` (a percent of the sheep) and `BOT_TABLE_EATS` set them, 2026-11-06. Bold rows are the even ones; the one match that ran to the clock was in the 36-eat row of 4 v 4.

| Line-up | Sheep | Fold share | Eats required | Sheepdogs win | Wolves win | Mean length | Shortest to longest | Mean eats | Mean in the Fold |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 v 1 | 100 | 25% | 9 | 7 | 3 | 300 s | 194 to 372 s | 7 | 24 |
| **1 v 1** | **80** | **30%** | **9** | **6** | **4** | **326 s** | **271 to 372 s** | **7** | **23** |
| 1 v 1 | 100 | 40% | 9 | 0 | 10 | 371 s | 366 to 374 s | 9 | 28 |
| 2 v 2 | 150 | 35% | 16 | 7 | 3 | 295 s | 224 to 388 s | 13 | 49 |
| **2 v 2** | **150** | **40%** | **16** | **5** | **5** | **313 s** | **247 to 388 s** | **14** | **53** |
| 2 v 2 | 150 | 40% | 18 | 8 | 2 | 328 s | 247 to 432 s | 14 | 56 |
| 4 v 4 | 250 | 40% | 32 | 8 | 2 | 300 s | 209 to 416 s | 25 | 97 |
| **4 v 4** | **250** | **45%** | **32** | **5** | **5** | **328 s** | **224 to 416 s** | **28** | **105** |
| 4 v 4 | 250 | 45% | 36 | 8 | 2 | 351 s | 224 to 480 s | 29 | 108 |
| 1 v 7 | 400 | 40% | 56 | 0 | 10 | 365 s | 334 to 373 s | 56 | 48 |
| 7 v 1 | 100 | 40% | 9 | 10 | 0 | 176 s | 102 to 291 s | 4 | 40 |

## Packed sheep (T15, T20)

Before and after sheep rest shoulder to shoulder in clumps of 8 to 24 and bodies part them (R1.2200 to R1.2205). `cargo run --release -p tool_core --example bot_table 10`, 2026-11-06. Bots are not people: they fetched lone sheep slowly, and clumps took that away.

| Line-up | Sheepdogs win, before | Sheepdogs win, after | Wolves win, before | Wolves win, after | Mean length, before | Mean length, after | Mean eats, before | Mean eats, after |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 v 1 | 6 | 10 | 4 | 0 | 326 s | 100 s | 7 | 2 |
| 2 v 2 | 5 | 10 | 5 | 0 | 313 s | 146 s | 14 | 6 |
| 4 v 4 | 5 | 10 | 5 | 0 | 328 s | 206 s | 28 | 16 |
| 1 v 7 | 0 | 0 | 10 | 10 | 365 s | 380 s | 56 | 56 |
| 7 v 1 | 10 | 10 | 0 | 0 | 143 s | 59 s | 3 | 1 |

The same table after, with `BOT_TABLE_FOLD` raised: Sheepdogs' wins and Wolves' wins out of 10, then the mean length. The Eats required stay as the table sets them.

| Fold share | 1 v 1 | 2 v 2 | 4 v 4 |
| --- | --- | --- | --- |
| 50% | 10 to 0, 192 s | 10 to 0, 190 s | 10 to 0, 229 s |
| 60% | 10 to 0, 270 s | 10 to 0, 259 s | 9 to 1, 333 s |
| 70% | 10 to 0, 316 s | 5 to 5, 347 s | 8 to 2, 390 s |
| 80% | 9 to 1, 398 s | 4 to 6, 375 s | 5 to 5, 452 s |

| What | Command | Result | When |
| --- | --- | --- | --- |
| Sheep stuck on the real map | `cargo test -p tool_core --test edges -- --nocapture --test-threads=1` | 0 of 5112 over 852 runs, 0 left within 3 m of a tip | 2026-11-06 |
| How tight sheep rest, real map, 100 and 250 sheep, after 3 calm minutes | a throwaway probe | Nearest neighbor median 0.98 to 1.01 m, p90 1.19 to 1.24 m. 8 to 10 clumps at 100 sheep, and sizes hold from the start | 2026-11-06 |
| 12-tick replay, native, before and after on the same busy machine, the better of two runs | `cargo run --release -p tool_core --example bench` | 100 sheep worst scene 0.34 ms before, 0.36 ms after. 400 sheep 1.49 ms before, 1.48 ms after. A run at load 15 reached 5.8 ms after and 1.9 ms before, once each | 2026-11-06 |

## Not measured

Devices, browsers and conditions no measurement here covers.

- Map generation and collision in WebAssembly: the numbers above are native.
- The hillside in the three browsers, and on any machine but this one. The whole frame sits behind a driver stack a player won't have, so treat its 6.7 ms as this machine's only.
- `World::step` under a page, a first load, or a browser other than V8: the sim's figures are node after 12,000 warm-up ticks, on a busy desktop, and a cold page sees a slower first moment.
- The sim figures above the two `Match::step` rows: those are the movement, the flock and the fright alone.
