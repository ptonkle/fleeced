//! Dressing: heather, bracken, gorse, bushes, small rocks, thistles and
//! boggy patches, so no stretch of the hillside looks bare from the play
//! camera. Picture only. Nothing here reaches a `Map`, so no body meets it,
//! and `table::DRESS_TALLEST` keeps it below a crouched Wolf's shoulder.

use crate::fx::{self, mul, Fx, ONE, TURN};
use crate::map::{table, Map};
use crate::mesh::{ring, Shape, COAT, OWN};
use crate::render::geom::{v3, Affine, V3};
use crate::render::Instance;

use super::cm;
use super::field::{to_segment, Field};
use super::noise::{fbm, lattice, lerp};
use super::palette::{self as pal, mix, Tone};
use super::shapes;

type P = (Fx, Fx);

/// A sort of dressing. The thin ones come last: a far square leaves them out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Heather,
    Bracken,
    Gorse,
    Bush,
    Rock,
    Thistle,
    Rushes,
}

impl Kind {
    pub const ALL: [Kind; 7] = [
        Kind::Heather,
        Kind::Bracken,
        Kind::Gorse,
        Kind::Bush,
        Kind::Rock,
        Kind::Thistle,
        Kind::Rushes,
    ];

    pub fn thin(self) -> bool {
        matches!(self, Kind::Thistle | Kind::Rushes)
    }
}

/// One thing on the ground.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dressing {
    pub kind: Kind,
    pub at: P,
    pub wide: Fx,
    pub tall: Fx,
    pub yaw: i32,
    pub tone: Tone,
}

impl Dressing {
    /// Its place, standing on ground at height `y`.
    pub fn instance(&self, y: Fx) -> Instance {
        let (c, s) = (fx::cos(self.yaw), fx::sin(self.yaw));
        // a stone's middle is its origin, and a plant's foot is
        let foot = if self.kind == Kind::Rock {
            self.tall / 4
        } else {
            -cm(3)
        };
        Instance {
            coat: self.tone,
            ..Instance::still(Affine {
                c0: v3(c, 0, s).scale(self.wide),
                c1: v3(0, self.tall, 0),
                c2: v3(-s, 0, c).scale(self.wide),
                t: v3(self.at.0, y + foot, self.at.1),
            })
        }
    }
}

/// Is a spot clear of everything a body needs to reach or read?
fn clear_at(map: &Map, p: P, gap_clear: Fx) -> bool {
    let far = |q: P, d: Fx| fx::length(p.0 - q.0, p.1 - q.1) > d;
    map.walls
        .iter()
        .all(|w| to_segment(p, w.a, w.b) > table::DRESS_WALL_CLEAR)
        && map.every_gap().all(|g| far(g.center, gap_clear))
        && map
            .trees
            .iter()
            .all(|t| far((t.x, t.z), t.trunk + table::DRESS_PROP_CLEAR))
        && map
            .cairns
            .iter()
            .all(|c| far((c.x, c.z), c.tiers[0].radius + table::DRESS_PROP_CLEAR))
        && map
            .slabs
            .iter()
            .all(|s| far(s.center(), s.reach() + table::DRESS_PROP_CLEAR))
        && map.mouths().iter().all(|&m| far(m, table::DRESS_MOUTH_CLEAR))
        && map
            .mounds
            .iter()
            .chain(map.spare_mounds.iter())
            .all(|m| far((m.x, m.z), table::DRESS_MOUND_CLEAR))
        && !map.in_fold(p.0, p.1)
}

/// What grows at `p`, from patches of noise and the spot's own draw: heather
/// and bracken run in stands, and the rest is thinly mixed.
fn kind_at(seed: u32, p: P, t: Fx) -> Kind {
    let heather = fbm(seed + 701, p.0, p.1, 21, 2);
    let bracken = fbm(seed + 733, p.0, p.1, 21, 2);
    if heather > cm(56) && t < cm(84) {
        return Kind::Heather;
    }
    if bracken > cm(56) && t < cm(84) {
        return Kind::Bracken;
    }
    match t * 100 / ONE {
        0..=23 => Kind::Rock,
        24..=47 => Kind::Thistle,
        48..=61 => Kind::Gorse,
        62..=71 => Kind::Bush,
        72..=87 => Kind::Heather,
        _ => Kind::Bracken,
    }
}

fn tone_of(kind: Kind, t: Fx) -> Tone {
    match kind {
        Kind::Heather if t < cm(55) => mix(pal::HEATHER_DARK, pal::HEATHER, t),
        Kind::Heather => mix(pal::HEATHER, pal::HEATHER_BLOOM, t),
        Kind::Bracken => mix(pal::BRACKEN_GREEN, pal::BRACKEN_TURNED, t / 2),
        Kind::Gorse => mix(pal::GORSE, pal::LEAF, t),
        Kind::Bush => mix(pal::LEAF, pal::ROWAN_LIT, t),
        Kind::Rock => mix(pal::WHIN, pal::GRANITE, t),
        Kind::Thistle | Kind::Rushes => [ONE; 3],
    }
}

/// One thing of `kind` at `p`, sized and turned by `roll`.
fn one(kind: Kind, p: P, roll: impl Fn(u32) -> Fx) -> Dressing {
    let ((wide_lo, wide_hi), (tall_lo, tall_hi)) = table::DRESS_SIZES[kind as usize];
    Dressing {
        kind,
        at: p,
        wide: lerp(cm(wide_lo), cm(wide_hi), roll(10)),
        tall: lerp(cm(tall_lo), cm(tall_hi), roll(11)),
        yaw: mul(roll(12), TURN),
        tone: tone_of(kind, roll(13)),
    }
}

/// Every piece of dressing for this map. `bogs` are the wet patches: nothing
/// stands in one, and rushes ring it.
pub fn scatter(map: &Map, seed: u32, bogs: &[(P, Fx)]) -> Vec<Dressing> {
    let mut out = Vec::new();
    let cell = table::DRESS_CELL;
    let m = table::TERRAIN_MARGIN;
    let across = |len: Fx| (len + 2 * m) / cell;
    let in_any_bog = |p: P| {
        bogs.iter()
            .any(|&(q, b)| fx::length(p.0 - q.0, p.1 - q.1) < b + cm(60))
    };
    for r in 0..across(map.width()) {
        for c in 0..across(map.length()) {
            let d = |n: u32| lattice(seed.wrapping_add(700 + n), c, r);
            let p = (-m + mul(d(1), cell) + c * cell, -m + mul(d(2), cell) + r * cell);
            let fill = table::DRESS_FILL[!map.inside(p.0, p.1) as usize];
            if d(0) * 100 / ONE >= fill || in_any_bog(p) || !clear_at(map, p, table::DRESS_GAP_CLEAR) {
                continue;
            }
            let kind = kind_at(seed, p, d(3));
            out.push(one(kind, p, d));
            // heather and bracken grow in drifts, not one by one
            if matches!(kind, Kind::Heather | Kind::Bracken) {
                for k in 0..table::DRESS_DRIFT {
                    let e = |n: u32| lattice(seed.wrapping_add(780 + n), c * 5 + k, r);
                    let a = mul(e(1), TURN);
                    let reach = lerp(cm(90), cm(250), e(2));
                    let q = (p.0 + mul(fx::cos(a), reach), p.1 + mul(fx::sin(a), reach));
                    if clear_at(map, q, table::DRESS_GAP_CLEAR) && !in_any_bog(q) {
                        out.push(one(kind, q, e));
                    }
                }
            }
        }
    }
    for (b, &(q, radius)) in bogs.iter().enumerate() {
        let tufts = (radius >> 15).max(4);
        for k in 0..tufts {
            let d = |n: u32| lattice(seed.wrapping_add(760 + n), b as i32, k);
            let angle = k * TURN / tufts + mul(d(1), TURN / 8);
            let reach = radius + cm(30) + mul(d(2), cm(60));
            let p = (q.0 + mul(fx::cos(angle), reach), q.1 + mul(fx::sin(angle), reach));
            if clear_at(map, p, table::DRESS_GAP_CLEAR) {
                out.push(one(Kind::Rushes, p, d));
            }
        }
    }
    out
}

/// The boggy patches: dark wet ground lying on the hill, ringed with moss.
/// The mesh, and each patch as a bare circle for the grass.
pub fn bogs(map: &Map, f: &Field, seed: u32) -> (Shape, Vec<(P, Fx)>) {
    let mut s = Shape::new();
    let mut wet = Vec::new();
    let cell = table::BOG_CELL;
    for r in 0..map.width() / cell {
        for c in 0..map.length() / cell {
            let d = |n: u32| lattice(seed.wrapping_add(800 + n), c, r);
            let p = (mul(d(1), cell) + c * cell, mul(d(2), cell) + r * cell);
            let radius = lerp(table::BOG_RADIUS.0, table::BOG_RADIUS.1, d(3));
            let room = radius + table::DRESS_WALL_CLEAR;
            let far = |q: P, r: Fx| fx::length(p.0 - q.0, p.1 - q.1) > room + r;
            if d(0) * 100 / ONE >= table::BOG_CHANCE
                || !map.inside(p.0, p.1)
                || !clear_at(map, p, table::BOG_GAP_CLEAR + radius)
                || map.walls.iter().any(|w| to_segment(p, w.a, w.b) < room)
                || !map.trees.iter().all(|t| far((t.x, t.z), t.trunk))
                || !map.cairns.iter().all(|t| far((t.x, t.z), t.tiers[0].radius))
                || !map.mouths().iter().all(|&m| far(m, 0))
                || !map.mounds.iter().all(|m| far((m.x, m.z), 0))
            {
                continue;
            }
            bog(&mut s, f, p, radius, |n| {
                lattice(seed.wrapping_add(830 + n), c, r)
            });
            wet.push((p, radius));
        }
    }
    (s, wet)
}

fn bog(s: &mut Shape, f: &Field, p: P, radius: Fx, roll: impl Fn(u32) -> Fx) {
    const RIM: i32 = 12;
    let up = v3(0, ONE, 0);
    let lift = cm(5);
    let on_ground = |x: Fx, z: Fx| v3(x, f.height_at(x, z) + lift, z);
    let spot = |k: i32, part: Fx| {
        let angle = k * TURN / RIM;
        let lobed = mul(radius, lerp(cm(70), cm(115), roll(k.rem_euclid(RIM) as u32)));
        let r = mul(lobed, part);
        on_ground(p.0 + mul(fx::cos(angle), r), p.1 + mul(fx::sin(angle), r))
    };
    let middle = on_ground(p.0, p.1);
    let water = pal::BOG_WATER;
    let rim = mix(pal::PEAT, pal::BOG_MOSS, cm(55));
    s.brush([ONE; 3], OWN, 0);
    for a in 0..RIM {
        let b = a + 1;
        let (ia, ib) = (spot(a, cm(50)), spot(b, cm(50)));
        let (oa, ob) = (spot(a, ONE), spot(b, ONE));
        s.painted_tri([middle, ib, ia], [up; 3], [water; 3]);
        s.painted_tri([ia, ib, ob], [up; 3], [water, water, rim]);
        s.painted_tri([ia, ob, oa], [up; 3], [water, rim, rim]);
    }
}

/// Lobes of leaves, dark at the foot and lit on top. Coat-colored, so each
/// bush takes its instance's tone.
fn lobes(s: &mut Shape, list: &[(V3, V3)]) {
    let first = s.drawn();
    s.brush([ONE; 3], COAT, 0);
    for &(at, radii) in list {
        s.ball(at, radii, &Affine::IDENTITY, 0);
    }
    s.tint_from(first, |p| {
        let v = lerp(cm(48), cm(116), p.y.clamp(0, ONE));
        [v, v, v]
    });
}

fn heather() -> Shape {
    let mut s = Shape::new();
    s.flat = true;
    lobes(
        &mut s,
        &[
            (v3(-cm(22), cm(44), cm(6)), v3(cm(26), cm(56), cm(24))),
            (v3(cm(20), cm(38), -cm(14)), v3(cm(24), cm(50), cm(24))),
            (v3(cm(4), cm(30), cm(24)), v3(cm(22), cm(42), cm(20))),
            (v3(cm(2), cm(52), -cm(4)), v3(cm(18), cm(46), cm(18))),
        ],
    );
    s
}

fn bracken() -> Shape {
    let mut s = Shape::new();
    s.flat = true;
    s.brush([ONE; 3], COAT, 0);
    for k in 0..7 {
        let a = k * TURN / 7 + TURN / 30 * (k % 3);
        let (c, si) = (fx::cos(a), fx::sin(a));
        let reach = cm(34) + cm(6) * (k % 3);
        let bend = v3(
            mul(c, reach * 3 / 5),
            cm(92) - cm(8) * (k % 2),
            mul(si, reach * 3 / 5),
        );
        let tip = v3(mul(c, reach + cm(14)), cm(44), mul(si, reach + cm(14)));
        let side = v3(-si, 0, c);
        s.lock(v3(0, 0, 0), bend, cm(16), cm(3), side);
        s.lock(bend, tip, cm(22), cm(3), side);
    }
    s
}

fn gorse(seed: u32) -> Shape {
    let mut s = Shape::new();
    lobes(
        &mut s,
        &[
            (v3(-cm(14), cm(46), cm(8)), v3(cm(36), cm(54), cm(34))),
            (v3(cm(20), cm(40), -cm(10)), v3(cm(30), cm(50), cm(32))),
            (v3(cm(2), cm(50), -cm(22)), v3(cm(24), cm(46), cm(22))),
            (v3(cm(4), cm(34), cm(28)), v3(cm(24), cm(40), cm(20))),
        ],
    );
    // its flowers keep their own yellow, whatever the instance's tone
    s.brush(pal::GORSE_FLOWER, OWN, 0);
    for k in 0..14 {
        let d = |n: u32| lattice(seed.wrapping_add(n), k, 41);
        let a = mul(d(1), TURN);
        let out = lerp(cm(26), cm(44), d(2));
        let at = v3(
            mul(fx::cos(a), out),
            lerp(cm(46), cm(92), d(3)),
            mul(fx::sin(a), out),
        );
        s.ball(at, v3(cm(7), cm(7), cm(7)), &Affine::IDENTITY, 0);
    }
    s
}

fn bush() -> Shape {
    let mut s = Shape::new();
    lobes(
        &mut s,
        &[
            (v3(0, cm(52), 0), v3(cm(42), cm(48), cm(42))),
            (v3(-cm(22), cm(30), cm(12)), v3(cm(28), cm(30), cm(26))),
            (v3(cm(24), cm(28), -cm(10)), v3(cm(26), cm(28), cm(26))),
        ],
    );
    s
}

fn thistle() -> Shape {
    let mut s = Shape::new();
    let side = v3(ONE, 0, 0);
    for (lean, top, head) in [
        (v3(0, 0, 0), cm(84), cm(16)),
        (v3(cm(22), 0, cm(8)), cm(66), cm(13)),
        (v3(-cm(14), 0, -cm(18)), cm(58), cm(12)),
    ] {
        let tip = lean.add(v3(0, top, 0));
        s.brush(pal::THISTLE_LEAF, OWN, 0);
        s.tube(&[ring(v3(0, 0, 0), cm(3)), ring(tip, cm(2))], 4, side);
        s.spike(v3(0, cm(14), 0), lean.add(v3(cm(12), cm(34), 0)), cm(5), 3, side);
        s.ball(
            tip.add(v3(0, cm(2), 0)),
            v3(head * 3 / 4, head * 3 / 4, head * 3 / 4),
            &Affine::IDENTITY,
            0,
        );
        s.brush(pal::THISTLE_HEAD, OWN, 0);
        s.ball(
            tip.add(v3(0, head * 5 / 6, 0)),
            v3(head / 2, head / 3, head / 2),
            &Affine::IDENTITY,
            0,
        );
    }
    s
}

fn rushes() -> Shape {
    let mut s = Shape::new();
    s.flat = true;
    let side = v3(ONE, 0, 0);
    s.brush([ONE; 3], OWN, 0);
    for k in 0..9 {
        let a = k * TURN / 9;
        let out = cm(32) + cm(5) * (k % 3);
        let foot = v3(mul(fx::cos(a), cm(12)), 0, mul(fx::sin(a), cm(12)));
        let tip = v3(
            mul(fx::cos(a), out),
            cm(76) + cm(10) * (k % 3),
            mul(fx::sin(a), out),
        );
        let first = s.drawn();
        s.spike(foot, tip, cm(3), 3, side);
        s.tint_from(first, |p| mix(pal::RUSH_FOOT, pal::RUSH_TIP, p.y.clamp(0, ONE)));
    }
    s
}

/// A kind's mesh: a unit wide and at most a unit tall, its foot at the origin.
pub fn prefab(kind: Kind, seed: u32) -> Shape {
    match kind {
        Kind::Heather => heather(),
        Kind::Bracken => bracken(),
        Kind::Gorse => gorse(seed),
        Kind::Bush => bush(),
        Kind::Rock => shapes::stone(seed + 71, 0, cm(30)),
        Kind::Thistle => thistle(),
        Kind::Rushes => rushes(),
    }
}

/// What far squares draw for anything solid-looking: a low dome.
pub fn far_prefab() -> Shape {
    let mut s = Shape::new();
    lobes(&mut s, &[(v3(0, cm(30), 0), v3(cm(50), cm(56), cm(50)))]);
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::from_int;

    fn dressed(seed: u64) -> (Map, Vec<Dressing>) {
        let map = Map::generate(seed);
        let f = Field::build(&map, seed as u32);
        let (_, wet) = bogs(&map, &f, seed as u32);
        let list = scatter(&map, seed as u32, &wet);
        (map, list)
    }

    #[test]
    fn a_loose_rock_is_a_pebble_not_a_stone_to_bump_into() {
        // past a Sheepdog's ankle or wider than its body it would look solid
        for seed in 0..6 {
            let list = dressed(seed).1;
            let rocks = list.iter().filter(|d| d.kind == Kind::Rock);
            assert!(rocks.clone().count() > 0);
            assert!(rocks.clone().all(|d| d.tall <= cm(16) && d.wide <= cm(38)));
        }
    }

    #[test]
    fn no_plant_or_pebble_grows_on_a_slab() {
        for seed in 0..6 {
            let (map, list) = dressed(seed);
            for s in &map.slabs {
                let c = s.center();
                assert!(list
                    .iter()
                    .all(|d| fx::length(d.at.0 - c.0, d.at.1 - c.1) > s.reach()));
            }
        }
    }

    #[test]
    fn nothing_in_the_dressing_stands_as_tall_as_a_crouched_wolf() {
        // a crouched Wolf's shoulder is 62 cm (`models::hiding`), and a
        // prefab's top is about a unit up
        assert!(table::DRESS_TALLEST < cm(62));
        for seed in 0..6 {
            for d in dressed(seed).1 {
                assert!(d.tall <= table::DRESS_TALLEST, "{d:?}");
            }
        }
        for kind in Kind::ALL {
            let top = prefab(kind, 1).finish().0.iter().map(|v| v.pos.y).max().unwrap();
            assert!(top <= ONE + cm(4), "{kind:?}: {top}");
        }
    }

    #[test]
    fn the_dressing_keeps_out_of_the_gaps_and_the_fold() {
        for seed in 0..6 {
            let (map, list) = dressed(seed);
            for d in &list {
                assert!(!map.in_fold(d.at.0, d.at.1), "{d:?}");
                for g in map.every_gap() {
                    let off = fx::length(d.at.0 - g.center.0, d.at.1 - g.center.1);
                    assert!(off > from_int(5), "{d:?}");
                }
                for w in &map.walls {
                    assert!(to_segment(d.at, w.a, w.b) > cm(150), "{d:?}");
                }
            }
        }
    }

    #[test]
    fn no_big_stretch_of_the_hillside_stands_bare() {
        // at 86% of 6 m squares a 24 m square holds about 14 things; six is
        // well under that, so only a square full of gap or tree falls short
        for seed in 0..6 {
            let (map, list) = dressed(seed);
            let (mut squares, mut empty) = (0, 0);
            for r in 0..7 {
                for c in 0..13 {
                    let lo = (from_int(c * 24), from_int(r * 24));
                    let mid = (lo.0 + from_int(12), lo.1 + from_int(12));
                    if map.field_at(mid.0, mid.1).is_none() {
                        continue;
                    }
                    squares += 1;
                    let inside = |d: &&Dressing| {
                        d.at.0 >= lo.0
                            && d.at.1 >= lo.1
                            && d.at.0 < lo.0 + from_int(24)
                            && d.at.1 < lo.1 + from_int(24)
                    };
                    empty += (list.iter().filter(inside).count() < 6) as i32;
                }
            }
            assert!(squares > 50 && empty <= 2, "seed {seed}: {empty} of {squares}");
        }
    }

    #[test]
    fn dressing_comes_from_the_seed() {
        let map = Map::generate(4);
        assert_eq!(scatter(&map, 4, &[]), scatter(&map, 4, &[]));
        assert_ne!(scatter(&map, 4, &[]), scatter(&map, 5, &[]));
    }

    #[test]
    fn a_bog_holds_no_plant() {
        let mut seen = 0;
        for seed in 0..8 {
            let map = Map::generate(seed);
            let f = Field::build(&map, seed as u32);
            let (_, wet) = bogs(&map, &f, seed as u32);
            seen += wet.len();
            let list = scatter(&map, seed as u32, &wet);
            for &(q, r) in &wet {
                assert!(list.iter().all(|d| fx::length(d.at.0 - q.0, d.at.1 - q.1) > r));
            }
        }
        assert!(seen >= 8, "{seen}");
    }
}
