# Build contract (agents read this)

Shared facts so parallel work merges. Change it only through the coordinator.

## Units and time

- Meters and seconds. `fx::Fx` (Q16.16) in the core. y is up, the ground plane is x, z. Angles in turns (`fx.rs`).
- 60 ticks a second (`crate::tune::TICKS_PER_SECOND`). Flock steering may be staggered across ticks.
- Q16.16 squares overflow past about 180 m and the map is longer than that: compare distances through `fx::length`, or in i64.
- Sheep can number up to 400 (the default grows with the wolf count; see BRIEF.md).
- Performance budget: one `World::step` with 100 sheep and 8 players at most 0.4 ms in wasm, so a 12-tick rollback replay stays under 5 ms; at 400 sheep at most 1 ms. `World` is `Clone` and cheap: flat Vecs of plain structs, no Rc, no pointers.

## Sizes (starting values; every balance number lives in a tune table)

| Thing | Length | Shoulder height |
| --- | --- | --- |
| Ewe (varies ±12%) | 1.05 m | 0.62 m |
| Ram (largest) | 1.15 m | 0.70 m |
| Lamb | 0.60 m | 0.38 m |
| Dog | 0.95 m | 0.56 m, crouched 0.38 m |
| Wolf | 1.25 m | 0.80 m, crouched 0.62 m (ewe height) |

Speeds (m/s): dog walk 9, sprint 16; wolf walk 8.4, sprint 14.8; crouched either 4.4; sheep graze drift 0.8, flock trot 4.8, flee 10 (twice the first numbers; the person: "a lot faster").

Map: Big is about 325 m by 175 m, Medium 244 by 131, Small 195 by 105, a walled hillside of three fields joined by gaps in the walls, the Fold at the top end. The rough shape stays the same on every seed; trees, cairns, holes and mounds move.

## Module owners

| Files | Owner | SECOND-ORDER IDs |
| --- | --- | --- |
| `crates/core/src/world.rs`, `flock.rs`, `body.rs`, `tune.rs`, `ground.rs` | sim | R1.100+ |
| `crates/core/src/map/` | map | R1.200+ |
| `web/index.html`, `web/app.js`, `web/style.css`, `web/net/`, `web/save/`, `data/patch-notes.json`, `data/strings.json`, `crates/core/src/api.rs`, `online.rs`, `patch_notes.rs`, `icon.rs`, `save.rs`, `scripts/`, `tests/browser/` | shell | R1.300+ |
| `web/gl.js`, `data/shaders/`, `crates/core/src/mesh.rs`, `models/`, `render.rs`, `web/gallery.html` | models | R1.400+ |
| `web/audio.js`, `crates/core/src/sound.rs`, `crates/core/src/tunes.rs`, `data/sound.json` | sound | R1.500+ |
| `crates/core/src/rules/` | rules | R1.600+ |
| `crates/core/src/scape/`, `data/shaders/grass.*`, `data/shaders/sky.*`, `data/wind.json` | hillside | R1.700+ |

Everyone may add one line to `crates/core/src/lib.rs` and rows to `tests/tags.toml`, `tests/breaks.json`, `GLOSSARY.md`. Keep those rows grouped by owner to keep merges trivial. SECOND-ORDER rows go in your own block at the file's end.

## Seam between sim and map

`ground.rs` (sim owns it) holds what the sim asks of a map, so neither waits on the other:

    pub trait Ground {
        fn height(&self, x: Fx, z: Fx) -> Fx;
        /// Push a circle out of walls, trees and cairns; returns the new x, z.
        fn push_out(&self, x: Fx, z: Fx, radius: Fx) -> (Fx, Fx);
    }

plus a `FlatField` for tests. `map::Map` implements it.

## Words

GLOSSARY.md wins: Sheepdog, Wolf, Fold (not pen), Carcass (not corpse), Tunnel with two mouths (not hole), powerup, Mound. Use them in identifiers too.

## Hard rules

No floats, HashMap, clocks or randomness in the core. Page JS holds no numbers and no rules (only `web/net/config.js` may hold transport numbers). Strings follow TONE.md and GLOSSARY.md; American spelling everywhere, including identifiers. Never Builder's default style pack. Check `web/`, `scripts/` and docs carry no `/home/<user>` paths.
