//! The fixed shape of the map: the outer wall, the two inner walls with
//! their gaps, and the Fold. A seed moves all of it a little and none of
//! it far, so every map reads as the same hillside.

use serde::Serialize;

use crate::fx::{self, Fx};
use crate::rng::Rng;

use super::size::MapSize;
use super::table;
use super::P;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum WallKind {
    /// Stops everyone, at any height.
    Outer,
    /// Divides the fields. Sheepdogs and Wolves can jump it; sheep can't.
    Inner,
    /// The Fold's front wall. A Sheepdog can jump it; sheep and Wolves can't.
    Fold,
}

/// One straight run of stone wall. `half` is half its thickness and
/// `height` its height above the ground.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Wall {
    pub a: P,
    pub b: P,
    pub half: Fx,
    pub height: Fx,
    pub kind: WallKind,
}

/// An opening in a wall. `a` and `b` are the wall's two faces across it, and
/// `width` is the clear distance between them. `wall` numbers the inner
/// walls from the low end; the Fold's own gap has the next number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Gap {
    pub wall: u8,
    pub center: P,
    pub a: P,
    pub b: P,
    pub width: Fx,
}

/// The Fold. `outline` is its walls in order, front corner to front corner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Fold {
    pub outline: Vec<P>,
    pub center: P,
    /// Where a folded sheep heads for.
    pub back: P,
    pub gap: Gap,
}

/// Everything this file builds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub outer: Vec<P>,
    pub walls: Vec<Wall>,
    /// The inner walls as whole lines, gaps included, low z to high z.
    pub dividers: Vec<Vec<P>>,
    pub gaps: Vec<Gap>,
    pub fold: Fold,
}

/// `a * b / c` through i64. `c` must not be zero.
fn scale(a: Fx, b: Fx, c: Fx) -> Fx {
    (a as i64 * b as i64 / c as i64) as Fx
}

fn nudged(rng: &mut Rng, base: Fx, by: Fx) -> Fx {
    base + rng.between(-by, by)
}

fn outer_wall(rng: &mut Rng, size: MapSize) -> Vec<P> {
    table::OUTER_OUTLINE
        .iter()
        .enumerate()
        .map(|(i, &(x, z))| {
            let in_fold = (table::FOLD_FIRST_CORNER..=table::FOLD_LAST_CORNER).contains(&i);
            let by = if in_fold {
                table::FOLD_NUDGE
            } else {
                // in step with the map, so each corner keeps its shallow angle
                size.up(table::OUTER_NUDGE)
            };
            let (x, z) = (fx::from_int(x), fx::from_int(z));
            let (x, z) = if in_fold {
                size.fold_corner(x, z)
            } else {
                (size.up(x), size.across(z))
            };
            (nudged(rng, x, by), nudged(rng, z, by))
        })
        .collect()
}

/// The z of the outer wall's low or high side at x.
fn wall_z_at(outer: &[P], size: MapSize, x: Fx, low: bool) -> Fx {
    let mid = size.width() / 2;
    for i in 0..outer.len() {
        let (a, b) = (outer[i], outer[(i + 1) % outer.len()]);
        let on_side = (a.1 < mid) == low && (b.1 < mid) == low;
        if on_side && a.0.min(b.0) <= x && x <= a.0.max(b.0) && a.0 != b.0 {
            return a.1 + scale(b.1 - a.1, x - a.0, b.0 - a.0);
        }
    }
    if low {
        0
    } else {
        size.width()
    }
}

/// Unit vector along (x, z).
fn unit(x: Fx, z: Fx) -> P {
    let n = fx::length(x, z);
    (fx::div(x, n), fx::div(z, n))
}

/// A gap, and the two ends of the wall either side of it. A wall's end is
/// rounded, so its line stops `half` short of the face.
fn opening(wall: u8, center: P, along: P, width: Fx) -> (Gap, P, P) {
    let at = |d: Fx| (center.0 + fx::mul(along.0, d), center.1 + fx::mul(along.1, d));
    let half_width = width / 2;
    let gap = Gap {
        wall,
        center,
        a: at(-half_width),
        b: at(half_width),
        width,
    };
    (
        gap,
        at(-half_width - table::WALL_HALF),
        at(half_width + table::WALL_HALF),
    )
}

fn wall(a: P, b: P, height: Fx, kind: WallKind) -> Wall {
    Wall {
        a,
        b,
        half: table::WALL_HALF,
        height,
        kind,
    }
}

/// A run of wall from `a` to `b`, bent once if it is long.
fn bend(rng: &mut Rng, a: P, b: P, line: &mut Vec<P>, runs: &mut Vec<(P, P)>) {
    if b.1 - a.1 >= table::DIVIDER_BEND_RUN {
        let off = rng.between(-table::DIVIDER_BEND, table::DIVIDER_BEND);
        let mid = ((a.0 + b.0) / 2 + off, (a.1 + b.1) / 2);
        runs.push((a, mid));
        runs.push((mid, b));
        line.push(mid);
    } else {
        runs.push((a, b));
    }
    line.push(b);
}

/// What the inner walls add up to while they are built.
#[derive(Default)]
struct Parts {
    walls: Vec<Wall>,
    dividers: Vec<Vec<P>>,
    gaps: Vec<Gap>,
}

/// One inner wall: its whole line, its runs of stone and its gaps. `side`
/// picks the slot of a single gap, and the two walls take opposite sides.
fn divider(rng: &mut Rng, size: MapSize, outer: &[P], number: u8, side: usize, parts: &mut Parts) {
    let base = size.up(table::DIVIDER_X[number as usize]);
    let slant = size.up(table::DIVIDER_SLANT);
    let low_x = nudged(rng, base, slant);
    let high_x = nudged(rng, base, slant);
    let low = (low_x, wall_z_at(outer, size, low_x, true));
    let high = (high_x, wall_z_at(outer, size, high_x, false));
    let line_x = |z: Fx| low.0 + scale(high.0 - low.0, z - low.1, high.1 - low.1);

    let two = rng.chance(table::TWO_GAPS_CHANCE.0, table::TWO_GAPS_CHANCE.1);
    let slots: Vec<Fx> = if two {
        table::TWO_GAP_SLOTS.to_vec()
    } else {
        vec![table::ONE_GAP_SLOTS[side]]
    };

    let mut line = vec![low];
    let mut runs = Vec::new();
    let mut start = low;
    for slot in slots {
        let z = nudged(rng, size.across(slot), size.over(table::GAP_MOVE));
        let center = (nudged(rng, line_x(z), table::GAP_OFFSET), z);
        let tilt = rng.between(-table::GAP_TILT, table::GAP_TILT);
        let width = rng.between(table::GAP_WIDTH_MIN, table::GAP_WIDTH_MAX);
        let (gap, near, far) = opening(number, center, unit(tilt, fx::ONE), width);
        parts.gaps.push(gap);
        bend(rng, start, near, &mut line, &mut runs);
        line.push(far);
        start = far;
    }
    bend(rng, start, high, &mut line, &mut runs);
    let height = table::INNER_WALL_HEIGHT;
    parts
        .walls
        .extend(runs.iter().map(|&(a, b)| wall(a, b, height, WallKind::Inner)));
    parts.dividers.push(line);
}

fn average(points: &[P]) -> P {
    let n = points.len() as i64;
    let sum = |f: fn(&P) -> Fx| points.iter().map(|p| f(p) as i64).sum::<i64>() / n;
    (sum(|p| p.0) as Fx, sum(|p| p.1) as Fx)
}

/// The Fold, with its gap in the front wall.
fn fold(rng: &mut Rng, outer: &[P], number: u8, walls: &mut Vec<Wall>) -> Fold {
    let outline = outer[table::FOLD_FIRST_CORNER..=table::FOLD_LAST_CORNER].to_vec();
    let (low, high) = (outline[0], outline[outline.len() - 1]);
    let along = unit(high.0 - low.0, high.1 - low.1);
    let middle = ((low.0 + high.0) / 2, (low.1 + high.1) / 2);
    let shift = rng.between(-table::FOLD_GAP_MOVE, table::FOLD_GAP_MOVE);
    let center = (
        middle.0 + fx::mul(along.0, shift),
        middle.1 + fx::mul(along.1, shift),
    );
    let (gap, near, far) = opening(number, center, along, table::FOLD_GAP_WIDTH);
    let height = table::FOLD_WALL_HEIGHT;
    walls.push(wall(low, near, height, WallKind::Fold));
    walls.push(wall(far, high, height, WallKind::Fold));

    let back_wall = average(&outline[1..outline.len() - 1]);
    let inward = unit(middle.0 - back_wall.0, middle.1 - back_wall.1);
    let back = (
        back_wall.0 + fx::mul(inward.0, table::FOLD_BACK_INSET),
        back_wall.1 + fx::mul(inward.1, table::FOLD_BACK_INSET),
    );
    Fold {
        center: average(&outline),
        back,
        gap,
        outline,
    }
}

/// The map's fixed shape for this seed.
pub fn build(rng: &mut Rng, size: MapSize) -> Layout {
    let outer = outer_wall(rng, size);
    let mut walls: Vec<Wall> = (0..outer.len())
        .map(|i| {
            let (a, b) = (outer[i], outer[(i + 1) % outer.len()]);
            wall(a, b, table::OUTER_WALL_HEIGHT, WallKind::Outer)
        })
        .collect();
    let mut parts = Parts::default();
    let side = rng.below(2) as usize;
    let count = table::DIVIDER_X.len();
    for number in 0..count {
        let on_side = if number % 2 == 0 { side } else { 1 - side };
        divider(rng, size, &outer, number as u8, on_side, &mut parts);
    }
    walls.append(&mut parts.walls);
    let fold = fold(rng, &outer, count as u8, &mut walls);
    Layout {
        outer,
        walls,
        dividers: parts.dividers,
        gaps: parts.gaps,
        fold,
    }
}
