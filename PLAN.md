# Plan

Stage 2, written by the agent from `BRIEF.md`. `B2 Require step-by-step justification`. Approved in advance (Autonomy).

| # | Decision | Reason | Test that shows it holds |
| --- | --- | --- | --- |
| 1 | **Units and time.** Meters and seconds in Q16.16, 60 ticks a second, y up (`notes/contract.md`). | Determinism under D2; one set of units for seven agents. | `fx.rs` and `ticks.rs` kit tests. |
| 2 | **`World` and `step`** (`world.rs`, `body.rs`). Flat, cheap to clone, inputs in, no I/O. | Rollback keeps one copy per unconfirmed tick. | A clone stepped with the same inputs matches byte for byte; the cost in `MEASUREMENTS.md` meets the contract's budget. |
| 3 | **Flock** (`flock.rs`). Strömbom-style shepherding on a uniform grid, up to 400 sheep. | Herding is the whole game. | Hand-worked tiny flocks; a scripted Sheepdog drives 30 sheep through a gap into the Fold. |
| 4 | **Map** (`map/`). One seeded hillside: three fields, gaps, the Fold, trees, cairns, tunnels, mounds. | The brief: same rough shape, different details. | Every seed in a few hundred has a flock route to the Fold; no prop in a gap; same seed, same map. |
| 5 | **Rules** (`rules/`). Small pure state machines with one tune table. | They can be built and tested before `World` exists, and the numbers may move. | Hand-worked: Fold target 100/10 → 75, 100/30 → 71, 250/40 → 188; defaults by wolf count; win checks; each powerup's timer. |
| 6 | **Match** joins `World`, `Map` and rules into one playable tick. | The seam the page and the session both call. | A scripted match reaches each ending: Wolves' eats, Fold target, time, Sudden death. |
| 7 | **Online.** `kits/online`'s rollback session over its WebRTC transport, up to 8 seats, `MAX_AHEAD` about 12 ticks. A departed seat becomes a bot at an agreed tick. | D23. Only inputs travel, so sheep are free. | The kit's required tests with Fleeced's `World` over a lagged wire; `check-online.sh`. |
| 8 | **Renderer.** WebGL2, instanced animals, an outline pass through walls. | 400 sheep, and outlines carry the one-sided information. | Frame cost at 400 sheep at 1920×1080 in `MEASUREMENTS.md`. |
| 9 | **Models.** Shetland sheep in the breed's real colors, border collie, wolf, carcass, costumes. | The real fleece colors are what lets a gray Wolf hide. | A shot of a crouched Wolf in a flock from the third-person camera: findable, not obvious. |
| 10 | **Hillside** (`scape/`). Ground, dry stone walls, trees, cairns, sky, a 3 s sunset for Howl, simple grass. | From real photos (`notes/hillside-look.md`). | Shots from the play camera; frame cost and bake time recorded. |
| 11 | **Camera and controls.** Third person, above and behind, pulled in by terrain; pointer lock; the brief's keys, plus C for crouch. | Alt can steal focus in a browser. | `input.js` kit tests; camera never inside a wall or hill. |
| 12 | **Targeting.** The core picks what's under the crosshair: a Wolf's red outline on the sheep it would eat; a Sheepdog gets no outline; a highlight on mounds and tunnel mouths. | The brief's central asymmetry. | Picks on small scenes; no outline data reaches a Sheepdog's view. |
| 13 | **HUD** in tartan and tweed: eat dial, stamina, powerup with icon and text, timer, counts per role, the Fold message. | KISS: only what the player acts on. | `information` tests; a shot per role. |
| 14 | **Powerups.** All twelve, wired into `Match`, each with an icon. | The brief's list, plus Stampede and Bell. | One small test per powerup through `Match`. |
| 15 | **Bots.** One simple level. Sheepdogs herd and hunt suspects; Wolves hide, eat, flee to tunnels. | Solo play, and the balance measurements of 19. | A bot match finishes with each team able to win. |
| 16 | **Sound.** All synthesized, including the two pipe tunes. | The brief. | Rendered WAVs for the person; note checks against the score. |
| 17 | **Shell.** Menus, settings, What's New, lobby with invite links, tab icon, volume. | Ported machinery; every game ships them. | Browser check; `tests/tags.toml`. |
| 18 | **Dog mode.** A final color pass for dog vision, red and green collapsed. | A setting the brief names. | Every HUD signal keeps a shape or brightness cue; a shot with it on. |
| 19 | **Balance.** Bot matches across 1v1, 2v2, 4v4, 1v7, 7v1, to tune the tables. | The numbers are spitballed (FL-2). | A table of win rates and match lengths in `MEASUREMENTS.md`. |
| 20 | **First deploy.** Local play, then ask, then `ptonkle/fleeced` and Pages. | FL-9. | The live site serves the new version before any live check. |

## Build order

Items 1 to 5, 8 to 10, 16 and 17 run now, in parallel. Then 6 and 7, then 11 to 15 in parallel, then 18 to 20.

## Open questions for the person

None: 35 were asked and answered (FL-Q1 to FL-Q35).

## Status

- [x] Approved in advance by the person on: 2026-11-06, in `BRIEF.md`'s Autonomy.
