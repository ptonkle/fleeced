//! The things that are not animals: the Carcass, the soft shadow under
//! everything, the lobby's plinth, and a test field for the gallery.

use crate::fx::{self, mul, ONE, TURN};
use crate::mesh::{rgb, ring, Rgb, Shape, COAT, OWN};
use crate::render::geom::{cm, deg, mm, v3, Affine, V3};

use super::sheep::{fleece, lumps, noise};

const WOOL: Rgb = rgb(236, 228, 210);
const BONE: Rgb = rgb(228, 220, 198);
const BONE_DARK: Rgb = rgb(186, 174, 150);
const BLOOD: Rgb = rgb(120, 22, 26);
const BLOOD_WET: Rgb = rgb(150, 30, 32);
const STONE: Rgb = rgb(150, 148, 140);
const STONE_DARK: Rgb = rgb(118, 116, 110);
const BRASS: Rgb = rgb(226, 182, 86);

/// One long bone: a shaft with a knob at each end.
fn long_bone(s: &mut Shape, a: V3, b: V3, r: i32) {
    s.color = BONE;
    s.tube(&[ring(a, r), ring(b, r)], 6, v3(0, ONE, 0));
    s.color = BONE_DARK;
    let knob = r * 17 / 10;
    s.ball(a, v3(knob, knob, knob), &Affine::IDENTITY, 0);
    s.ball(b, v3(knob, knob, knob), &Affine::IDENTITY, 0);
}

/// One rib: an arc from the spine down to the ground on one side.
fn rib(s: &mut Shape, z: i32, x: i32, wide: i32, tall: i32) {
    let steps = 6;
    let curve: Vec<_> = (0..=steps)
        .map(|k| {
            let a = fx::ratio(k, steps) / 4 + TURN / 64;
            let p = v3(mul(x, mul(fx::sin(a), wide)), mm(16) + mul(fx::cos(a), tall), z);
            ring(p, mm(10) - mm(1) * k / 2)
        })
        .collect();
    s.color = BONE;
    s.tube(&curve, 5, v3(0, 0, ONE));
}

/// A small pile of bones with a little blood: what is left of a sheep. A
/// tuft of its wool stays, in the instance's coat.
/// A tuft of wool on the wind, about a hand across.
pub fn tuft() -> Shape {
    let mut s = Shape::new();
    s.flat = true;
    s.brush(WOOL, OWN, 0);
    let puffs = [
        (v3(0, 0, 0), v3(cm(6), cm(5), cm(7))),
        (v3(cm(5), cm(2), cm(2)), v3(cm(4), cm(3), cm(4))),
        (v3(-cm(4), cm(1), -cm(4)), v3(cm(4), cm(4), cm(3))),
    ];
    for (at, size) in puffs {
        s.ball(at, size, &Affine::rot_y(deg(25)), 1);
    }
    s
}

pub fn carcass() -> Shape {
    let mut s = Shape::new();
    s.flat = true;
    // the blood first: two low pools, flat on the ground
    s.brush(BLOOD, OWN, 0);
    s.ball(
        v3(cm(4), mm(4), 0),
        v3(cm(24), mm(8), cm(19)),
        &Affine::rot_y(deg(20)),
        1,
    );
    s.color = BLOOD_WET;
    s.ball(
        v3(-cm(10), mm(6), cm(7)),
        v3(cm(9), mm(9), cm(7)),
        &Affine::rot_y(deg(50)),
        1,
    );
    s.ball(
        v3(cm(15), mm(6), -cm(9)),
        v3(cm(6), mm(8), cm(5)),
        &Affine::IDENTITY,
        0,
    );
    // the ribs, propped round the spine
    for k in 0..5 {
        let z = cm(-14) + cm(7) * k;
        let wide = cm(14) - (k - 2).abs() * cm(1);
        rib(&mut s, z, ONE, wide, cm(13));
        rib(&mut s, z, -ONE, wide, cm(13));
    }
    s.color = BONE_DARK;
    s.tube(
        &[
            ring(v3(0, cm(14), -cm(20)), mm(13)),
            ring(v3(mm(6), cm(15), cm(18)), mm(11)),
        ],
        6,
        v3(ONE, 0, 0),
    );
    // the skull, on its side
    s.color = BONE;
    let skull = v3(cm(26), cm(6), cm(19));
    s.ball(skull, v3(cm(6), mm(52), mm(66)), &Affine::rot_y(deg(35)), 1);
    let snout = skull.add(v3(cm(8), -mm(8), cm(9)));
    s.tube(&[ring(skull, mm(40)), ring(snout, mm(20))], 6, v3(0, ONE, 0));
    s.color = rgb(40, 34, 30);
    s.ball(
        skull.add(v3(mm(30), mm(32), mm(24))),
        v3(mm(13), mm(11), mm(13)),
        &Affine::IDENTITY,
        0,
    );
    // the long bones, thrown about
    long_bone(
        &mut s,
        v3(-cm(30), mm(20), -cm(6)),
        v3(-cm(10), mm(24), -cm(24)),
        mm(11),
    );
    long_bone(
        &mut s,
        v3(-cm(24), mm(20), cm(26)),
        v3(-cm(4), mm(36), cm(30)),
        mm(10),
    );
    long_bone(
        &mut s,
        v3(cm(14), mm(20), -cm(32)),
        v3(cm(34), mm(20), -cm(20)),
        mm(10),
    );
    // a tuft of its wool
    s.flat = false;
    s.brush([ONE; 3], COAT, 0);
    let tuft = v3(-cm(18), cm(5), cm(2));
    fleece(
        &mut s,
        tuft,
        v3(cm(9), cm(5), cm(7)),
        &lumps(tuft, v3(cm(6), cm(3), cm(5)), cm(3), 12, 11),
        2,
    );
    s
}

/// The soft shadow: a disc whose first color channel is its strength, dark
/// in the middle and gone at the rim. One unit across each way from middle.
pub fn shadow() -> Shape {
    let mut s = Shape::new();
    let bands = [(0, 100), (45, 86), (75, 42), (100, 0)];
    let around = 20;
    let at = |r: i32, k: i32| {
        let a = TURN * k / around;
        v3(mul(fx::sin(a), cm(r)), 0, mul(fx::cos(a), cm(r)))
    };
    let tone = |pct: i32| [cm(pct); 3];
    let up = [V3::UP; 3];
    for pair in bands.windows(2) {
        let ((r0, d0), (r1, d1)) = (pair[0], pair[1]);
        for k in 0..around {
            s.painted_tri(
                [at(r0, k), at(r1, k), at(r1, k + 1)],
                up,
                [tone(d0), tone(d1), tone(d1)],
            );
            if r0 > 0 {
                s.painted_tri(
                    [at(r0, k), at(r1, k + 1), at(r0, k + 1)],
                    up,
                    [tone(d0), tone(d1), tone(d0)],
                );
            }
        }
    }
    s
}

/// A small round stone plinth with a brass ring set round its top: your own
/// place in the lobby.
pub fn plinth() -> Shape {
    let mut s = Shape::new();
    s.flat = true;
    let side = v3(ONE, 0, 0);
    s.brush(STONE_DARK, OWN, 0);
    s.tube(
        &[
            ring(v3(0, 0, 0), cm(92)),
            ring(v3(0, cm(8), 0), cm(92)),
            ring(v3(0, cm(10), 0), cm(84)),
        ],
        18,
        side,
    );
    s.brush(STONE, OWN, 0);
    s.tube(
        &[ring(v3(0, cm(10), 0), cm(80)), ring(v3(0, cm(24), 0), cm(78))],
        18,
        side,
    );
    // the brass ring: a low band stood proud of the top edge
    s.brush(BRASS, OWN, 0);
    let around = 48;
    let mut band: Vec<_> = (0..=around)
        .map(|k| {
            let a = TURN * k / around;
            ring(
                v3(mul(fx::cos(a), cm(80)), cm(25), mul(fx::sin(a), cm(80))),
                mm(26),
            )
        })
        .collect();
    band[around as usize] = band[0];
    s.flat = false;
    s.tube(&band, 8, V3::UP);
    s
}

/// A flat field for the gallery: short grass in patches of green.
const GRASS: Rgb = rgb(114, 152, 73);

pub fn meadow() -> Shape {
    let mut s = Shape::new();
    s.flat = true;
    // one plain color, so nothing but the animals draws the eye
    s.color = GRASS;
    let cells = 60;
    let size = cm(400);
    let half = size * cells / 2;
    for i in 0..cells {
        for j in 0..cells {
            let p = |a: i32, b: i32| v3(size * (i + a) - half, 0, size * (j + b) - half);
            s.tri(p(0, 0), p(0, 1), p(1, 1));
            s.tri(p(0, 0), p(1, 1), p(1, 0));
        }
    }
    s
}

/// A short length of dry stone wall for the gallery's outline test: stacked
/// flat stones, 6 m long and 1.2 m high, running along x.
pub fn stones() -> Shape {
    let mut s = Shape::new();
    s.flat = true;
    for row in 0..5 {
        let stones_in_row = 13;
        for k in 0..stones_in_row {
            let n = noise(row * 57 + k * 13);
            let tone = 104 + (n >> 13) + (noise(k * row + 3) >> 14);
            let gray = rgb(150, 148, 140);
            s.brush(
                [gray[0] * tone / 100, gray[1] * tone / 100, gray[2] * tone / 100],
                OWN,
                0,
            );
            let x = cm(-300) + cm(50) * k + (row % 2) * cm(25) + mul(n, cm(6));
            let y = cm(14) + cm(24) * row;
            let tilt = Affine::rot_y(mul(n, deg(14)));
            let wide = cm(30) + mul(noise(k * 7 + row), cm(4));
            s.ball(v3(x, y, 0), v3(wide, cm(14), cm(26)), &tilt, 0);
        }
    }
    s
}
