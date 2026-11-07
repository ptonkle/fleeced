//! Where everyone begins. Sheepdogs start by the Fold, Wolves at tunnel
//! mouths, and sheep in large clumps through the fields.

use crate::fx::{self, Fx};
use crate::ground::Ground;
use crate::rng::Rng;

use super::props::apart;
use super::solids::root;
use super::{table, Map, P};

/// An arc of spots in front of the Fold's gap, facing down the hill.
pub fn sheepdog_starts(rng: &mut Rng, map: &Map) -> Vec<P> {
    let (cx, cz) = map.fold.gap.center;
    let count = table::SHEEPDOG_STARTS as i32;
    let half = table::SHEEPDOG_ARC_TURNS;
    (0..count)
        .map(|i| {
            let angle = -half + 2 * half * i / (count - 1);
            let r = rng.between(table::SHEEPDOG_ARC_RADIUS.0, table::SHEEPDOG_ARC_RADIUS.1);
            (cx - fx::mul(r, fx::cos(angle)), cz + fx::mul(r, fx::sin(angle)))
        })
        .collect()
}

/// Every mouth, the ones farthest from the Fold first, so a Wolf starts well
/// away from the Sheepdogs. Eight mouths for at most seven Wolves.
pub fn wolf_starts(map: &Map) -> Vec<P> {
    let mut mouths = map.mouths();
    let gap = map.fold.gap.center;
    mouths.sort_by_key(|m| std::cmp::Reverse(fx::length(m.0 - gap.0, m.1 - gap.1)));
    mouths
}

/// 0.381966 of a turn.
const GOLDEN_ANGLE: i32 = 25033;

/// A good place for a sheep of this field to start?
fn sheep_fits(map: &Map, p: P, field: usize) -> bool {
    map.field_at(p.0, p.1) == Some(field)
        && map.push_out(p.0, p.1, table::SHEEP_START_CLEAR) == p
        && map
            .gaps
            .iter()
            .all(|g| apart(g.center, p, table::SHEEP_GAP_CLEAR))
        && apart(map.fold.gap.center, p, table::SHEEP_FOLD_CLEAR)
        && map
            .mouths()
            .iter()
            .all(|&m| apart(m, p, table::SHEEP_MOUTH_CLEAR))
        && map
            .mounds
            .iter()
            .chain(&map.spare_mounds)
            .all(|m| apart((m.x, m.z), p, table::SHEEP_MOUND_CLEAR))
}

/// How many sheep the next clump holds.
fn clump_size(rng: &mut Rng) -> usize {
    let total = table::SHEEP_CLUMPS.iter().map(|c| c.1).sum();
    let mut roll = rng.below(total);
    for &(size, weight) in &table::SHEEP_CLUMPS {
        if roll < weight {
            return size;
        }
        roll -= weight;
    }
    table::SHEEP_CLUMPS[0].0
}

/// How many sheep a clump holds on average, in percent of a sheep.
fn clump_mean_percent() -> usize {
    let total: usize = table::SHEEP_CLUMPS.iter().map(|c| c.1 as usize).sum();
    let sheep: usize = table::SHEEP_CLUMPS.iter().map(|c| c.0 * c.1 as usize).sum();
    sheep * 100 / total
}

/// The even spacing for this many sheep in this field, in meters, scaled
/// down so random darts still land.
fn clump_spacing(map: &Map, field: usize, sheep: usize) -> Fx {
    let clumps = (sheep * 100 / clump_mean_percent()).max(1) as u64;
    let area = map.size.area(table::FIELD_AREA[field]) as u64;
    let even = root((area << 32) / clumps) as i32;
    (even as i64 * table::SHEEP_SPACING_PERCENT as i64 / 100) as Fx
}

/// `sheep` start spots in one field.
fn scatter(rng: &mut Rng, map: &Map, field: usize, sheep: usize, out: &mut Vec<P>) {
    let (lo, hi) = table::FIELD_SPAN[field];
    let (lo, hi) = (map.size.up(lo), map.size.up(hi));
    let mut spacing = clump_spacing(map, field, sheep);
    let mut clumps: Vec<P> = Vec::new();
    let mut placed = 0;
    let mut misses = 0;
    while placed < sheep {
        let p = (rng.between(lo, hi), rng.between(0, map.width()));
        let clear = map.push_out(p.0, p.1, table::SHEEP_CLUMP_CLEAR) == p;
        if !clear || !sheep_fits(map, p, field) || !clumps.iter().all(|&c| apart(c, p, spacing)) {
            misses += 1;
            if misses == table::PLACE_TRIES {
                misses = 0;
                spacing = spacing / 8 * 7;
            }
            continue;
        }
        misses = 0;
        clumps.push(p);
        out.push(p);
        placed += 1;
        // what would be too few for a clump joins this one
        let left = sheep - placed + 1;
        let size = clump_size(rng);
        let size = if left < size + table::SHEEP_CLUMPS[0].0 {
            left
        } else {
            size
        };
        let first = rng.below(fx::TURN as u32) as i32;
        let from = out.len() - 1;
        for k in 1..size as i32 {
            // the golden angle round, so each sheep lands in the widest space
            let angle = first + k * GOLDEN_ANGLE;
            let loose = rng.between(-table::SHEEP_CLUMP_LOOSE, table::SHEEP_CLUMP_LOOSE);
            let r = fx::mul(table::SHEEP_CLUMP_PACK, fx::sqrt(fx::from_int(k + 1))) + loose;
            let q = (p.0 + fx::mul(r, fx::cos(angle)), p.1 + fx::mul(r, fx::sin(angle)));
            let clear = out[from..].iter().all(|&o| apart(o, q, table::SHEEP_CLUMP_GAP));
            if clear && sheep_fits(map, q, field) {
                out.push(q);
                placed += 1;
            }
        }
    }
}

/// Start spots for a flock of `count`, the same spots every call for the
/// same map and count. Works from 1 sheep up; built for 100 to 400.
pub fn sheep_starts(map: &Map, count: usize) -> Vec<P> {
    let mut rng = Rng::new(map.sheep_seed);
    let mut out = Vec::with_capacity(count);
    let mut left = count;
    for field in 0..table::SHEEP_SHARE.len() {
        let share = if field + 1 == table::SHEEP_SHARE.len() {
            left
        } else {
            count * table::SHEEP_SHARE[field] / 100
        };
        left -= share;
        scatter(&mut rng, map, field, share, &mut out);
    }
    out
}
