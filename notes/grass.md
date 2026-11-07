# Grass, from Chumworld (sgilson7/chumworld at a644c57c, MIT, Sam Gilson: add a LICENSES.md row)

Chumworld uses no cards, no alpha, no per-blade instancing. It bakes opaque, vertex-colored three-sided prisms (7 triangles: three sides and a blunt top) into chunk meshes in distance tiers, and shades the ground itself as grass.

## The simplest version for Fleeced

Keep the ground shading, blades and tufts, and `data/wind.json`. Leave out the clump tier, chunk streaming, dither hand-over, moving origin, flowers, reeds. Bake once per match: 120 blade chunks + 32 tuft chunks. Draw the chunks overlapping each tier's distance band.

| Tier | Prisms | Placement | Distance |
| --- | --- | --- | --- |
| Blades | 3 spikes, 9 cm wide | 45 cm cell, 88% filled, 18 m chunks | full to 17 m, gone by 26 m |
| Tufts | 1-2, 28 cm wide | 1.2 m cell, 65%, 36 m chunks | in over 12-20 m, out over 62-78 m |
| Ground | fragment shading only | where the ground is green | everywhere |

The table is as built (R1.1103). Measured: 384k blades, 43k prisms, 28.7 MB for the whole map; the bake takes about a second. Blades are spikes of 3 triangles and 4 corners; the first estimate of 67k prisms read as a bare sheet from the play camera.

Short-grass values (Chumworld's in brackets): blade height 11-22 cm (22-42), lean 4-11 (6-16); tuft height 12-20 (24-36), lean 2-5 (4-10); foot sunk 2 cm (5); height of full bend 20 cm (50); bend 6 cm (11); `uSink` 0.3 m (1.2).

## Tricks worth keeping

- Each prism mixes a grass tone into the ground color under it (45% blades, 25% tufts). Low contrast is the main defense against shimmer.
- Keep widths at 9 and 28 cm: they're the anti-blink numbers. Anything under about 0.4 px blinked in a slow pan.
- Near darkening: 78% bright at 6 m, full by 18 m.
- Fade by each vertex's own distance, sinking instead of dithering: `w.y -= (1.0 - abs(f)) * uSink;`. Dither looked bad from above, and Fleeced's camera is above.
- Wind: four summed sines, the same in the grass and ground shaders, so sway and gusts line up. `data/wind.json`: wavelengths 46/23/71/13 m, speeds 5.2/3.9/6.4/3.0 m/s, shares 42/26/20/12%.
- Ground: sheen 10%, shadow 8%, troughs 12% dark from two crossing waves 3.3 m and 5.1 m long, each fading via `fwidth` before it's finer than a pixel. Green must beat red by 20/255 and blue by 54/255, from the hill pasture photos (R1.1101; Chumworld asked for 34 over both, which keeps an emerald).

## Traps

- Chunk pop-in was the top complaint ("theres never a moment where all the grass within the immediate viewable area with your eyes is rendered"), then "it still dissappears wayy to close to you". Vertex fade and ground shading fixed it. Let the ground shading carry the distance.
- Straight-up normals only match flat ground. Bake the ground's normal into each prism or grass lights differently from the slope.
- Give grass its own shader program with a plain `bend` attribute; Chumworld's normal-length trick isn't needed.
- Rollback: derive wind phases outside the rollback state, with `step.wrapping_mul(tick)`. Don't keep them in `World`.
- Feet are baked from the same triangles as the ground, so it must match the ground mesh's split.
- The view from above shows gaps. `grass_cell_cm` is the dial: 150 to 100 means 2.25× the triangles.
- No GPU time was ever recorded. Measure with `EXT_disjoint_timer_query_webgl2`.

## Files to copy from

`data/wind.json`; `crates/core/src/wind.rs` (not its tests; our `fx.rs` lacks `deg` and `TAU`); `data/shaders/scene.vert` band (65), wind (76), block 93-105; `scene.frag` wind (90), ground block (139-149); `crates/core/src/scatter.rs` band (147), band_fade (195), tufts (396), bend_from (725), blade_lit (742), GRASS and TUFT arms (1204, 1251); `terrain.rs` patch (1215), GroundPatch (1739) as a pattern; `crates/shim/src/lib.rs` floats.
