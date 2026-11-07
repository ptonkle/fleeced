//! Galloway dykes, stone by stone (notes/hillside-look.md). From the ground
//! up, as the photos show them: flat stones laid in one or two courses, a
//! double wall; a single row of big stones set on end, leaning, with daylight
//! in the chinks between them; then lumpy cope stones along the top, all
//! leaning the same way. At a gap the wall ends in a cheek of squarer stones
//! stacked in turn, long and short. A few loose stones lie at the foot.
//!
//! Each stone is an instance of a few shared stone meshes, so a wall costs
//! memory by the stone, not by the triangle.

use crate::fx::{self, Fx, ONE, TURN};
use crate::map::walls::{Gap, Wall};
use crate::map::Map;
use crate::rng::Rng;

use super::cm;
use super::noise;
use super::palette::{self, Tone};

/// What part of the wall a stone is, which picks its look.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Course {
    Flat,
    Upright,
    Cope,
    Cheek,
    Loose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stone {
    /// The middle of the stone: x, y, z.
    pub at: [Fx; 3],
    /// Its full size along the wall, upward and across the wall.
    pub size: [Fx; 3],
    /// The way along the wall, in turns round y from +x.
    pub yaw: i32,
    /// How far it leans along the wall, in turns.
    pub lean: i32,
    pub tone: Tone,
    pub course: Course,
    /// Which of the stone meshes it uses.
    pub shape: u8,
}

/// How many stone meshes there are for each look.
pub const ROUND_SHAPES: u8 = 4;

/// Where the wall's tiers stop, above the ground.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tiers {
    pub courses: i32,
    pub flat_top: Fx,
    pub lace_top: Fx,
}

/// A wall too low for two flat courses gets one, so its stones stay as chunky
/// as a tall wall's.
pub fn tiers(height: Fx) -> Tiers {
    Tiers {
        courses: if height >= cm(120) { 2 } else { 1 },
        flat_top: fx::mul(height, cm(32)),
        lace_top: fx::mul(height, cm(74)),
    }
}

fn along_run(w: &Wall) -> (Fx, Fx, Fx) {
    let (dx, dz) = (w.b.0 - w.a.0, w.b.1 - w.a.1);
    let len = fx::length(dx, dz);
    (fx::div(dx, len), fx::div(dz, len), len)
}

/// The angle of (x, z) from +x, in turns, from a rough octant table: stones
/// need no better, and the core has no arctangent.
pub fn angle_of(x: Fx, z: Fx) -> i32 {
    let (ax, az) = (x.abs() as i64, z.abs() as i64);
    let big = ax.max(az).max(1);
    let small = ax.min(az);
    // atan(t) in turns, for t in 0..=1: t/8 is within 0.2% of a turn
    let first = (small * (TURN as i64) / (8 * big)) as i32;
    let oct = if ax >= az { first } else { TURN / 4 - first };
    match (x >= 0, z >= 0) {
        (true, true) => oct,
        (false, true) => TURN / 2 - oct,
        (false, false) => TURN / 2 + oct,
        (true, false) => TURN - oct,
    }
}

/// How the ends of a run finish: a run stopped by a gap ends at the gap's
/// face, half a wall past the end of its line.
fn ends_at_gap(p: (Fx, Fx), w: &Wall, gaps: &[Gap]) -> bool {
    let near = |q: (Fx, Fx)| fx::length(p.0 - q.0, p.1 - q.1) <= w.half + cm(5);
    gaps.iter().any(|g| near(g.a) || near(g.b))
}

/// A stone's tone. The photos show runs of a wall going warmer or bluer
/// together, so `warmth` comes from noise along the wall.
fn stone_tone(rng: &mut Rng, warmth: Fx, pale: bool) -> Tone {
    let roll = rng.below(100);
    let base = if pale {
        match roll {
            0..=54 => palette::WHIN_PALE,
            55..=79 => palette::WHIN,
            80..=92 => palette::WHIN_WARM,
            _ => palette::WHIN_MOSSY,
        }
    } else {
        match roll {
            0..=39 => palette::WHIN,
            40..=64 => palette::WHIN_DARK,
            65..=84 => palette::WHIN_WARM,
            85..=94 => palette::WHIN_TAN,
            _ => palette::WHIN_MOSSY,
        }
    };
    let warm = palette::mix(base, palette::WHIN_TAN, fx::mul(warmth, cm(45)));
    palette::pct(warm, rng.between(90, 112))
}

struct Run<'a> {
    w: &'a Wall,
    map: &'a Map,
    dir: (Fx, Fx),
    yaw: i32,
    seed: u32,
}

impl Run<'_> {
    /// The point `s` along the run and `off` to its left.
    fn point(&self, s: Fx, off: Fx) -> (Fx, Fx) {
        (
            self.w.a.0 + fx::mul(self.dir.0, s) - fx::mul(self.dir.1, off),
            self.w.a.1 + fx::mul(self.dir.1, s) + fx::mul(self.dir.0, off),
        )
    }

    fn warmth(&self, s: Fx) -> Fx {
        let (x, z) = self.point(s, 0);
        noise::band(cm(35), cm(70), noise::value(self.seed, x, z, 21))
    }

    #[allow(clippy::too_many_arguments)]
    fn stone(
        &self,
        s: Fx,
        off: Fx,
        y: Fx,
        size: [Fx; 3],
        lean: i32,
        tone: Tone,
        course: Course,
        shape: u8,
    ) -> Stone {
        let (x, z) = self.point(s, off);
        Stone {
            at: [x, self.map.terrain.at(x, z) + y, z],
            size,
            yaw: self.yaw,
            lean,
            tone,
            course,
            shape,
        }
    }
}

/// The wall's stones along one run. `from` and `to` are how far the stones
/// run along it, and they may pass its ends.
fn lay_run(run: &Run, tier: Tiers, from: Fx, to: Fx, rng: &mut Rng, out: &mut Vec<Stone>) {
    let height = run.w.height;

    // The flat courses: a double wall, its two faces laid separately.
    let course_h = tier.flat_top / tier.courses;
    for side in [-1, 1] {
        for course in 0..tier.courses {
            let offset = if course == 0 { cm(24) } else { cm(19) };
            let mut s = from - rng.between(0, cm(26));
            while s < to {
                let long = rng.between(cm(38), cm(70));
                let tall = fx::mul(course_h, rng.between(cm(92), cm(112)));
                let deep = rng.between(cm(34), cm(46));
                let mid = (s + long / 2).clamp(from, to);
                let y = course_h * course + tall / 2 - cm(3);
                let across = side * (offset + rng.between(-cm(2), cm(3)));
                let tone = stone_tone(rng, run.warmth(mid), false);
                out.push(run.stone(
                    mid,
                    across,
                    y,
                    [long, tall, deep],
                    rng.between(-TURN / 90, TURN / 90),
                    tone,
                    Course::Flat,
                    rng.below(ROUND_SHAPES as u32) as u8,
                ));
                s += fx::mul(long, cm(66));
            }
        }
    }

    // The lace: two tight rows of stones on end, leaning a little in runs,
    // packed so they overlap with no daylight between them. It sits a little
    // into the course below.
    let lace_h = tier.lace_top - tier.flat_top;
    let mut s = from + rng.between(0, cm(20));
    let mut lean_way = if rng.chance(1, 2) { 1 } else { -1 };
    let mut row = 1;
    while s < to {
        let wide = rng.between(cm(26), cm(38));
        let tall = fx::mul(lace_h, rng.between(cm(112), cm(134)));
        let deep = rng.between(cm(34), cm(46));
        if rng.chance(1, 4) {
            lean_way = -lean_way;
        }
        let lean = lean_way * rng.between(TURN / 120, TURN / 40);
        let mid = s + wide / 2;
        let y = tier.flat_top + tall / 2 - cm(6);
        row = -row;
        let across = row * rng.between(cm(8), cm(14));
        let tone = stone_tone(rng, run.warmth(mid), false);
        out.push(run.stone(
            mid,
            across,
            y,
            [wide, tall, deep],
            lean,
            tone,
            Course::Upright,
            rng.below(ROUND_SHAPES as u32) as u8,
        ));
        s += fx::mul(wide, cm(46));
    }

    // The cope: lumpy stones across the top, leaning the same way as a set.
    let cope_h = height - tier.lace_top;
    let mut s = from + rng.between(0, cm(15));
    let cope_lean = if rng.chance(1, 2) { TURN / 36 } else { -TURN / 36 };
    while s < to {
        let wide = rng.between(cm(28), cm(42));
        let tall = fx::mul(cope_h, rng.between(cm(104), cm(130)));
        let deep = rng.between(cm(38), cm(52));
        let mid = s + wide / 2;
        let y = height - cope_h + tall / 2 - cm(4);
        let tone = stone_tone(rng, run.warmth(mid), true);
        out.push(run.stone(
            mid,
            rng.between(-cm(3), cm(3)),
            y,
            [wide, tall, deep],
            cope_lean + rng.between(-TURN / 60, TURN / 60),
            tone,
            Course::Cope,
            rng.below(ROUND_SHAPES as u32) as u8,
        ));
        s += fx::mul(wide, cm(68));
    }

    // A few loose stones at the foot.
    let mut s = from + rng.between(0, ONE * 6);
    while s < to {
        let size = rng.between(cm(12), cm(26));
        let across = rng.between(cm(62), cm(130)) * if rng.chance(1, 2) { 1 } else { -1 };
        let tone = stone_tone(rng, run.warmth(s), false);
        out.push(run.stone(
            s,
            across,
            size / 4,
            [size, size * 3 / 5, size * 4 / 5],
            rng.between(-TURN / 24, TURN / 24),
            tone,
            Course::Loose,
            rng.below(ROUND_SHAPES as u32) as u8,
        ));
        s += rng.between(ONE * 3, ONE * 14);
    }
}

/// The cheek at a wall's end: squarer stones stacked in turn, long and
/// short, to the wall's full height, with a big cope stone to finish.
/// `face` is where the gap's face stands, and `inward` is +1 or -1, the way
/// back along the run.
fn cheek(run: &Run, face: Fx, inward: i32, rng: &mut Rng, out: &mut Vec<Stone>) {
    let height = run.w.height;
    let layers = (height / cm(30)).max(3);
    let tall = height / layers;
    for layer in 0..layers {
        let long = if layer % 2 == 0 { cm(58) } else { cm(44) };
        let deep = if layer % 2 == 0 { cm(66) } else { cm(84) };
        let top = layer == layers - 1;
        let tall = if top { tall + cm(8) } else { tall };
        let y = tall / 2 + (height / layers) * layer - cm(2);
        let tone = stone_tone(rng, run.warmth(face), top);
        out.push(run.stone(
            face + inward * (long / 2),
            rng.between(-cm(2), cm(2)),
            y,
            [long + rng.between(0, cm(6)), tall, deep + rng.between(0, cm(8))],
            0,
            tone,
            Course::Cheek,
            ROUND_SHAPES,
        ));
    }
}

/// Every stone of the map's walls, built from a seed so a map always gets
/// the same walls.
pub fn stones(map: &Map, seed: u32) -> Vec<Stone> {
    let mut out = Vec::new();
    let mut all_gaps: Vec<Gap> = map.gaps.clone();
    all_gaps.push(map.fold.gap);
    for (i, w) in map.walls.iter().enumerate() {
        let (dx, dz, len) = along_run(w);
        let run = Run {
            w,
            map,
            dir: (dx, dz),
            yaw: angle_of(dx, dz),
            seed: seed ^ 0x51,
        };
        let mut rng = Rng::new((seed as u64) << 16 | i as u64);
        let (gap_a, gap_b) = (ends_at_gap(w.a, w, &all_gaps), ends_at_gap(w.b, w, &all_gaps));
        // past a corner, a little; to the gap's face at a gap
        let from = if gap_a { -w.half + cm(40) } else { -cm(25) };
        let to = if gap_b {
            len + w.half - cm(40)
        } else {
            len + cm(25)
        };
        lay_run(&run, tiers(w.height), from, to, &mut rng, &mut out);
        if gap_a {
            cheek(&run, -w.half, 1, &mut rng, &mut out);
        }
        if gap_b {
            cheek(&run, len + w.half, -1, &mut rng, &mut out);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::from_int;
    use crate::map::walls::WallKind;

    fn distance_to(p: (Fx, Fx), a: (Fx, Fx), b: (Fx, Fx)) -> Fx {
        let (ex, ez) = ((b.0 - a.0) as i64, (b.1 - a.1) as i64);
        let (px, pz) = ((p.0 - a.0) as i64, (p.1 - a.1) as i64);
        let len2 = (ex * ex + ez * ez).max(1);
        let t = (((px * ex + pz * ez) << 8) / (len2 >> 8)).clamp(0, ONE as i64);
        let q = (a.0 + ((ex * t) >> 16) as Fx, a.1 + ((ez * t) >> 16) as Fx);
        fx::length(p.0 - q.0, p.1 - q.1)
    }

    #[test]
    fn angle_of_lands_on_the_compass_points() {
        assert_eq!(angle_of(ONE, 0), 0);
        assert_eq!(angle_of(0, ONE), TURN / 4);
        assert_eq!(angle_of(-ONE, 0), TURN / 2);
        assert_eq!(angle_of(0, -ONE), TURN * 3 / 4);
        assert_eq!(angle_of(ONE, ONE), TURN / 8);
    }

    #[test]
    fn a_tall_wall_gets_two_flat_courses_and_a_low_one_gets_one() {
        // the map's outer wall is 1.5 m and its inner walls 0.9 m
        assert_eq!(tiers(cm(150)).courses, 2);
        assert_eq!(tiers(cm(90)).courses, 1);
        let t = tiers(cm(150));
        assert!(0 < t.flat_top && t.flat_top < t.lace_top && t.lace_top < cm(150));
    }

    #[test]
    fn a_wall_is_built_in_its_three_tiers_from_the_ground_up() {
        let map = Map::generate(3);
        let stones = stones(&map, 3);
        let ground = |s: &Stone| map.terrain.at(s.at[0], s.at[2]);
        let mean = |c: Course| {
            let list: Vec<Fx> = stones
                .iter()
                .filter(|s| s.course == c)
                .map(|s| s.at[1] - ground(s))
                .collect();
            list.iter().map(|&v| v as i64).sum::<i64>() / list.len() as i64
        };
        let (flat, upright, cope) = (mean(Course::Flat), mean(Course::Upright), mean(Course::Cope));
        assert!(flat < upright && upright < cope, "{flat} {upright} {cope}");
        // lean only on the stones set on end and the copes
        assert!(stones
            .iter()
            .any(|s| s.course == Course::Upright && s.lean.abs() >= TURN / 45));
        assert!(stones.iter().all(|s| s.course != Course::Cheek || s.lean == 0));
    }

    #[test]
    fn every_gap_face_gets_a_cheek_and_no_gap_is_blocked() {
        for seed in [1u64, 5, 9] {
            let map = Map::generate(seed);
            let stones = stones(&map, seed as u32);
            let mut gaps = map.gaps.clone();
            gaps.push(map.fold.gap);
            for g in &gaps {
                for face in [g.a, g.b] {
                    let near = stones.iter().filter(|s| {
                        s.course == Course::Cheek && fx::length(s.at[0] - face.0, s.at[2] - face.1) < cm(80)
                    });
                    assert!(near.count() >= 3, "seed {seed}: a gap face has no cheek");
                }
                // nothing but a loose stone stands in the opening
                let inset = g.width / 2 - cm(45);
                let blockers = stones.iter().filter(|s| {
                    s.course != Course::Loose
                        && fx::length(s.at[0] - g.center.0, s.at[2] - g.center.1) < inset
                });
                assert_eq!(blockers.count(), 0, "seed {seed}");
            }
        }
    }

    #[test]
    fn a_stone_stays_on_its_wall() {
        let map = Map::generate(7);
        let stones = stones(&map, 7);
        // stones are laid from runs of an inner or outer wall: none strays
        // more than the wall's half width and a stone's own size from one
        for s in stones.iter().filter(|s| s.course != Course::Loose) {
            let nearest = map
                .walls
                .iter()
                .map(|w| distance_to((s.at[0], s.at[2]), w.a, w.b))
                .min()
                .unwrap();
            assert!(nearest < cm(115), "{:?}: {}", s.course, nearest);
        }
    }

    #[test]
    fn the_same_seed_lays_the_same_stones_and_a_new_seed_does_not() {
        let map = Map::generate(11);
        assert_eq!(stones(&map, 11), stones(&map, 11));
        assert_ne!(stones(&map, 11), stones(&map, 12));
    }

    #[test]
    fn the_map_stays_cheap_at_under_two_dozen_stones_a_meter() {
        let map = Map::generate(2);
        let total = stones(&map, 2).len() as i64;
        let meters: i64 = map
            .walls
            .iter()
            .map(|w| fx::length(w.b.0 - w.a.0, w.b.1 - w.a.1) as i64 >> 16)
            .sum();
        assert!(
            meters > from_int(0) as i64 && total / meters < 24,
            "{total} stones, {meters} m"
        );
        assert!(map.walls.iter().any(|w| w.kind == WallKind::Outer));
    }
}
