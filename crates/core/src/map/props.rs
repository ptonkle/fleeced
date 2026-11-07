//! What a seed moves freely: trees, cairns, slabs, tunnels and mounds. Hard
//! clearances (walls, gaps) keep the flock's route open whatever the dice
//! say; spacing between props eases only if a spot can't be found.

use serde::Serialize;

use crate::fx::{self, Fx};
use crate::ground::Ground;
use crate::rng::Rng;

use super::{table, terrain, Map, P};

/// A lone tree. `trunk` is the solid part; `crown` and `height` are for the
/// picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Tree {
    pub x: Fx,
    pub z: Fx,
    pub trunk: Fx,
    pub crown: Fx,
    pub height: Fx,
}

/// One step of a cairn: a level disc whose top is at height `top`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Tier {
    pub radius: Fx,
    pub top: Fx,
}

/// A stepped pile of stones, tiers listed from the bottom up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Cairn {
    pub x: Fx,
    pub z: Fx,
    pub tiers: Vec<Tier>,
}

impl Cairn {
    /// The radius that stops a body with feet this high, or `None` when the
    /// feet clear every step. No feet means a body that can't climb.
    pub fn stopping_radius(&self, feet: Option<Fx>) -> Option<Fx> {
        let Some(feet) = feet else {
            return Some(self.tiers[0].radius);
        };
        let reach = feet + table::CAIRN_STEP_UP;
        self.tiers.iter().find(|t| t.top > reach).map(|t| t.radius)
    }

    /// The top of the highest step over (x, z), if any.
    pub fn top_at(&self, x: Fx, z: Fx) -> Option<Fx> {
        let (dx, dz) = ((x - self.x) as i64, (z - self.z) as i64);
        let d2 = dx * dx + dz * dz;
        self.tiers
            .iter()
            .rev()
            .find(|t| d2 < t.radius as i64 * t.radius as i64)
            .map(|t| t.top)
    }
}

/// A long low stone through the turf: a capsule from `a` to `b`, `half`
/// wide on each side and `top` above the ground under it. The rules and the
/// picture both read it, so what shows is what stops a body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Slab {
    pub a: P,
    pub b: P,
    pub half: Fx,
    pub top: Fx,
}

impl Slab {
    pub fn center(&self) -> P {
        ((self.a.0 + self.b.0) / 2, (self.a.1 + self.b.1) / 2)
    }

    /// From the center to the farthest point of the outline.
    pub fn reach(&self) -> Fx {
        fx::length(self.b.0 - self.a.0, self.b.1 - self.a.1) / 2 + self.half
    }
}

/// A Wolf-only passage. A Wolf goes in at one mouth and comes out at the
/// other, always the same pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Tunnel {
    pub a: P,
    pub b: P,
}

impl Tunnel {
    /// The mouth a Wolf going in at `mouth` comes out of.
    pub fn other(&self, mouth: P) -> P {
        if mouth == self.a {
            self.b
        } else {
            self.a
        }
    }
}

/// A small heap of dirt at the base of a tree or cairn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Mound {
    pub x: Fx,
    pub z: Fx,
}

/// Are the two points at least `d` apart? Compared in i64: the map is longer
/// than an Fx square can hold.
pub fn apart(a: P, b: P, d: Fx) -> bool {
    let (dx, dz) = ((a.0 - b.0) as i64, (a.1 - b.1) as i64);
    dx * dx + dz * dz >= d as i64 * d as i64
}

/// Does a circle this wide at `p` touch no wall? At this stage the map's
/// solids hold only its walls.
fn clear_of_walls(map: &Map, p: P, radius: Fx) -> bool {
    map.push_out(p.0, p.1, radius) == p
}

fn clear_of_gaps(map: &Map, p: P, d: Fx) -> bool {
    map.every_gap().all(|g| apart(g.center, p, d))
}

/// Is `p` this far from where every inner wall meets the outer wall?
fn clear_of_wall_ends(map: &Map, p: P, d: Fx) -> bool {
    let ends = map
        .dividers
        .iter()
        .flat_map(|line| [line[0], line[line.len() - 1]]);
    ends.into_iter().all(|end| apart(end, p, d))
}

/// A random point in the field, or `None` when the draw misses it.
fn draw(rng: &mut Rng, map: &Map, field: usize) -> Option<P> {
    let (lo, hi) = table::FIELD_SPAN[field];
    let p = (
        rng.between(map.size.up(lo), map.size.up(hi)),
        rng.between(0, map.width()),
    );
    (map.field_at(p.0, p.1) == Some(field)).then_some(p)
}

/// How much of the full spacing applies in each round of tries, in Q16: all
/// of it at first, an eighth less each round that finds nothing.
fn eases() -> impl Iterator<Item = Fx> {
    (0..table::PLACE_ROUNDS).map(|round| fx::ratio(8 - round, 8))
}

/// A random spot in the field that `ok` accepts at the current easing, or
/// `None` when the field has no room left: a prop with no good spot stays out.
fn spot(rng: &mut Rng, map: &Map, field: usize, mut ok: impl FnMut(P, Fx) -> bool) -> Option<P> {
    for ease in eases() {
        for _ in 0..table::PLACE_TRIES {
            if let Some(p) = draw(rng, map, field) {
                if ok(p, ease) {
                    return Some(p);
                }
            }
        }
    }
    None
}

fn place_trees(rng: &mut Rng, map: &Map) -> Vec<Tree> {
    let mut trees: Vec<Tree> = Vec::new();
    for (field, &quota) in map.size.row().trees.iter().enumerate() {
        for _ in 0..quota {
            let trunk = rng.between(table::TRUNK_RADIUS.0, table::TRUNK_RADIUS.1);
            let found = spot(rng, map, field, |p, ease| {
                let spacing = fx::mul(table::TREE_SPACING, ease);
                tree_room(map, p, trunk) && trees.iter().all(|t| apart((t.x, t.z), p, spacing))
            });
            let Some(p) = found else {
                continue;
            };
            trees.push(Tree {
                x: p.0,
                z: p.1,
                trunk,
                crown: rng.between(table::CROWN_RADIUS.0, table::CROWN_RADIUS.1),
                height: rng.between(table::TREE_HEIGHT.0, table::TREE_HEIGHT.1),
            });
        }
    }
    place_groves(rng, map, &mut trees);
    trees
}

/// A tree spot, clear of everything a lone tree keeps clear of.
fn tree_room(map: &Map, p: P, trunk: Fx) -> bool {
    clear_of_walls(map, p, trunk + table::PROP_WALL_CLEAR)
        && clear_of_gaps(map, p, table::PROP_GAP_CLEAR)
        && clear_of_wall_ends(map, p, table::PROP_WALL_END_CLEAR)
}

/// Small groups of young trees. A group's first tree keeps a lone tree's
/// spacing; the rest stand close by, each a route's width from every trunk.
/// A group that runs out of room stays smaller.
fn place_groves(rng: &mut Rng, map: &Map, trees: &mut Vec<Tree>) {
    let young = |rng: &mut Rng, p: P, trunk: Fx| Tree {
        x: p.0,
        z: p.1,
        trunk,
        crown: rng.between(table::GROVE_CROWN.0, table::GROVE_CROWN.1),
        height: rng.between(table::GROVE_HEIGHT.0, table::GROVE_HEIGHT.1),
    };
    for (field, &quota) in map.size.row().groves.iter().enumerate() {
        for _ in 0..quota {
            let trunk = rng.between(table::GROVE_TRUNK.0, table::GROVE_TRUNK.1);
            let first = spot(rng, map, field, |p, ease| {
                let spacing = fx::mul(table::TREE_SPACING, ease);
                tree_room(map, p, trunk) && trees.iter().all(|t| apart((t.x, t.z), p, spacing))
            });
            let Some(first) = first else {
                continue;
            };
            let tree = young(rng, first, trunk);
            trees.push(tree);
            let more = rng.between(table::GROVE_TREES.0, table::GROVE_TREES.1) - 1;
            for _ in 0..more {
                let trunk = rng.between(table::GROVE_TRUNK.0, table::GROVE_TRUNK.1);
                for _ in 0..table::GROVE_TRIES {
                    let angle = rng.below(fx::TURN as u32) as i32;
                    let d = table::GROVE_CLEAR + 2 * trunk + rng.between(0, table::GROVE_SPREAD);
                    let p = (
                        first.0 + fx::mul(d, fx::cos(angle)),
                        first.1 + fx::mul(d, fx::sin(angle)),
                    );
                    let room = |t: &Tree| apart((t.x, t.z), p, t.trunk + trunk + table::GROVE_CLEAR);
                    if map.field_at(p.0, p.1) == Some(field)
                        && tree_room(map, p, trunk)
                        && trees.iter().all(room)
                    {
                        let tree = young(rng, p, trunk);
                        trees.push(tree);
                        break;
                    }
                }
            }
        }
    }
}

/// A cairn's steps, counted up from `base`, the ground at its center. On a
/// slope the lowest step stands taller on the downhill side.
fn tiers(rng: &mut Rng, base: Fx) -> Vec<Tier> {
    let count = rng.between(table::CAIRN_STEPS.0, table::CAIRN_STEPS.1);
    (0..count)
        .map(|i| Tier {
            radius: table::CAIRN_TOP_RADIUS + table::CAIRN_TREAD * (count - 1 - i),
            top: base + table::CAIRN_RISE * (i + 1),
        })
        .collect()
}

fn place_cairns(rng: &mut Rng, map: &Map, trees: &[Tree]) -> Vec<Cairn> {
    let mut cairns: Vec<Cairn> = Vec::new();
    // the widest a cairn gets
    let reach = table::CAIRN_TOP_RADIUS + table::CAIRN_TREAD * (table::CAIRN_STEPS.1 - 1);
    for (field, &quota) in map.size.row().cairns.iter().enumerate() {
        for _ in 0..quota {
            let mut best: Option<(Fx, P)> = None;
            for _ in 0..table::CAIRN_CANDIDATES {
                let p = spot(rng, map, field, |p, ease| {
                    let spacing = fx::mul(table::CAIRN_SPACING, ease);
                    clear_of_walls(map, p, reach + table::PROP_WALL_CLEAR)
                        && clear_of_gaps(map, p, table::PROP_GAP_CLEAR)
                        && clear_of_wall_ends(map, p, table::PROP_WALL_END_CLEAR)
                        && cairns.iter().all(|c| apart((c.x, c.z), p, spacing))
                        && trees
                            .iter()
                            .all(|t| apart((t.x, t.z), p, table::CAIRN_TREE_CLEAR))
                });
                let Some(p) = p else {
                    continue;
                };
                let rise = map.terrain_height(p.0, p.1) - terrain::ramp(p.0);
                if best.is_none_or(|(b, _)| rise > b) {
                    best = Some((rise, p));
                }
            }
            let Some((_, p)) = best else {
                continue;
            };
            let base = map.terrain_height(p.0, p.1);
            cairns.push(Cairn {
                x: p.0,
                z: p.1,
                tiers: tiers(rng, base),
            });
        }
    }
    cairns
}

/// Is a mouth at `p` clear of walls, gaps, the Fold's gap and the props?
fn mouth_fits(map: &Map, p: P, trees: &[Tree], cairns: &[Cairn]) -> bool {
    clear_of_walls(map, p, table::MOUTH_WALL_CLEAR)
        && clear_of_gaps(map, p, table::MOUTH_GAP_CLEAR)
        && apart(map.fold.gap.center, p, table::MOUTH_FOLD_CLEAR)
        && trees
            .iter()
            .all(|t| apart((t.x, t.z), p, t.trunk + table::MOUTH_PROP_CLEAR))
        && cairns
            .iter()
            .all(|c| apart((c.x, c.z), p, c.tiers[0].radius + table::MOUTH_PROP_CLEAR))
}

fn place_tunnels(rng: &mut Rng, map: &Map, trees: &[Tree], cairns: &[Cairn]) -> Vec<Tunnel> {
    let mut tunnels: Vec<Tunnel> = Vec::new();
    for &(from, to) in map.size.row().tunnels.iter() {
        let mouths: Vec<P> = tunnels.iter().flat_map(|t| [t.a, t.b]).collect();
        let spaced = |p: P, ease: Fx| {
            let spacing = fx::mul(map.size.up(table::MOUTH_SPACING), ease);
            mouths.iter().all(|&m| apart(m, p, spacing))
        };
        let fits = |p: P, ease: Fx| mouth_fits(map, p, trees, cairns) && spaced(p, ease);
        // the two mouths are drawn together: a good first mouth may leave no
        // second one far enough away
        let mut pair = None;
        'tries: for ease in eases() {
            for _ in 0..table::PLACE_TRIES {
                let (Some(a), Some(b)) = (draw(rng, map, from), draw(rng, map, to)) else {
                    continue;
                };
                let far_enough = apart(a, b, map.size.up(table::TUNNEL_LENGTH_MIN));
                if far_enough && fits(a, ease) && fits(b, ease) {
                    pair = Some(Tunnel { a, b });
                    break 'tries;
                }
            }
        }
        tunnels.extend(pair);
    }
    tunnels
}

/// A tree or cairn a mound can sit by.
struct Anchor {
    center: P,
    /// The surface of its trunk or base.
    radius: Fx,
    /// The side its first mound took, in turns.
    angle: i32,
}

fn shuffled(rng: &mut Rng, list: &mut [Anchor]) {
    for i in (1..list.len()).rev() {
        list.swap(i, rng.below(i as u32 + 1) as usize);
    }
}

/// The mound spots: those out at the start, then the spares in the order
/// they come up. The fields take turns, and a tree or cairn takes a second
/// spot, on its far side, once every one in its field has one.
fn place_mounds(rng: &mut Rng, map: &Map) -> (Vec<Mound>, Vec<Mound>) {
    let mut by_field: [Vec<Anchor>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let trees = map.trees.iter().map(|t| ((t.x, t.z), t.trunk));
    let cairns = map.cairns.iter().map(|c| ((c.x, c.z), c.tiers[0].radius));
    for (center, radius) in trees.chain(cairns) {
        if let Some(field) = map.field_at(center.0, center.1) {
            by_field[field].push(Anchor {
                center,
                radius,
                angle: rng.below(fx::TURN as u32) as i32,
            });
        }
    }
    let mut taken = [0usize; 3];
    let mut field = rng.below(3) as usize;
    let mut spots = Vec::new();
    let total = table::MOUNDS_AT_START + table::SPARE_MOUNDS;
    for _ in 0..total {
        // a field with no tree or cairn left for a mound passes its turn on
        for _ in 0..3 {
            if taken[field] < 2 * by_field[field].len() {
                break;
            }
            field = (field + 1) % 3;
        }
        if taken[field] == 0 {
            shuffled(rng, &mut by_field[field]);
        }
        let list = &by_field[field];
        let anchor = &list[taken[field] % list.len()];
        let second = taken[field] >= list.len();
        let turn = if second {
            fx::HALF_TURN + rng.between(-fx::TURN / 12, fx::TURN / 12)
        } else {
            0
        };
        let d = anchor.radius + table::MOUND_OFFSET;
        spots.push(Mound {
            x: anchor.center.0 + fx::mul(d, fx::cos(anchor.angle + turn)),
            z: anchor.center.1 + fx::mul(d, fx::sin(anchor.angle + turn)),
        });
        taken[field] += 1;
        field = (field + 1) % 3;
    }
    let spare = spots.split_off(table::MOUNDS_AT_START);
    (spots, spare)
}

/// Is a slab reaching `reach` from `p` clear of all that a body or the flock
/// needs? Mounds need no check: each sits by a tree or cairn, and slabs keep
/// well clear of those. The map's solids must hold only its walls.
fn slab_fits(map: &Map, slabs: &[Slab], p: P, reach: Fx) -> bool {
    let prop_clear = reach + table::SLAB_PROP_CLEAR;
    map.field_at(p.0, p.1).is_some()
        && clear_of_walls(map, p, reach + table::PROP_WALL_CLEAR)
        && clear_of_gaps(map, p, reach + table::PROP_GAP_CLEAR)
        && clear_of_wall_ends(map, p, reach + table::PROP_WALL_END_CLEAR)
        && map
            .trees
            .iter()
            .all(|t| apart((t.x, t.z), p, t.trunk + prop_clear))
        && map
            .cairns
            .iter()
            .all(|c| apart((c.x, c.z), p, c.tiers[0].radius + prop_clear))
        && map
            .mouths()
            .iter()
            .all(|&m| apart(m, p, reach + table::SLAB_MOUTH_CLEAR))
        && slabs.iter().all(|s| apart(s.center(), p, s.reach() + prop_clear))
}

/// The slabs, all lying roughly the one way, as the rock's grain does.
fn place_slabs(rng: &mut Rng, map: &Map) -> Vec<Slab> {
    let mut slabs: Vec<Slab> = Vec::new();
    let grain = fx::TURN / 3 + rng.between(0, fx::TURN / 8);
    for row in 0..map.width() / table::SLAB_CELL {
        for col in 0..map.length() / table::SLAB_CELL {
            let roll = rng.below(100);
            let p = (
                table::SLAB_CELL * col + rng.between(0, table::SLAB_CELL),
                table::SLAB_CELL * row + rng.between(0, table::SLAB_CELL),
            );
            let long = rng.between(table::SLAB_REACH.0, table::SLAB_REACH.1);
            let half = rng.between(table::SLAB_HALF.0, table::SLAB_HALF.1);
            let top = rng.between(table::SLAB_TOP.0, table::SLAB_TOP.1);
            let turn = grain + rng.between(-fx::TURN / 28, fx::TURN / 28);
            if roll >= table::SLAB_FILL || !slab_fits(map, &slabs, p, long + half) {
                continue;
            }
            let (dx, dz) = (fx::mul(long, fx::cos(turn)), fx::mul(long, fx::sin(turn)));
            slabs.push(Slab {
                a: (p.0 - dx, p.1 - dz),
                b: (p.0 + dx, p.1 + dz),
                half,
                top,
            });
        }
    }
    slabs
}

/// Fills in the map's props. The map's solids must hold its walls.
pub fn place(map: &mut Map, rng: &mut Rng) {
    map.trees = place_trees(rng, map);
    map.cairns = place_cairns(rng, map, &map.trees);
    map.tunnels = place_tunnels(rng, map, &map.trees, &map.cairns);
    (map.mounds, map.spare_mounds) = place_mounds(rng, map);
    map.slabs = place_slabs(rng, map);
}
