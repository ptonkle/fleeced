//! The one map: a walled Scottish hillside of three fields joined by gaps,
//! with the Fold at the top end. `Map::sized` builds it from a seed and a Map
//! size. The rough shape stays the same on every seed; the seed moves the gaps
//! a little and everything else freely.
//!
//! x runs up the hill, from 0 at the low end to `length` at the Fold's back
//! wall; z runs across, from 0 to `width`. y is up. Meters, in `Fx`.
//!
//! A `Map` is built once at match start and never changes, so a `World`
//! borrows it rather than cloning it.

pub mod proof;
pub mod props;
pub mod size;
pub mod solids;
pub mod starts;
pub mod table;
pub mod terrain;
pub mod walls;

use serde::Serialize;

use crate::fx::Fx;
use crate::ground::{Ground, FOLD_FIELD};
use crate::rng::Rng;

use props::{Cairn, Mound, Slab, Tree, Tunnel};
pub use size::MapSize;
use solids::{polygon_contains, root, Part, Solid, Solids, Zone};
use terrain::Heightfield;
use walls::{Fold, Gap, Wall, WallKind};

/// A point on the ground plane: x, z.
pub type P = (Fx, Fx);

/// How a body meets the map's solids. `feet` is the height of the body's
/// feet, so a jump can clear what walking cannot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mover {
    /// A sheep outside the Fold. Sheep walk: every wall stops them, and they
    /// use the gaps.
    Sheep,
    /// A sheep in the Fold. It stays in: the Fold's gap is shut to it.
    FoldedSheep,
    /// Clears inner walls and the Fold's front wall at their tops.
    Sheepdog { feet: Fx },
    /// Clears inner walls at their tops. The Fold stops it at any height.
    Wolf { feet: Fx },
    /// A line of sight, not a body: what stops a sheep stops it, but a low
    /// slab is looked over.
    Sight,
}

impl Mover {
    fn feet(self) -> Option<Fx> {
        match self {
            Mover::Sheep | Mover::FoldedSheep | Mover::Sight => None,
            Mover::Sheepdog { feet } | Mover::Wolf { feet } => Some(feet),
        }
    }
}

/// How many rounds `push_out_as` takes to settle a body in a corner.
const PUSH_ROUNDS: usize = 4;
/// A pushed body lands this far past contact (in Fx units, about 0.1 mm), so
/// rounding cannot leave it touching and a second push does nothing.
const SKIN: i64 = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Map {
    pub seed: u64,
    pub size: MapSize,
    pub terrain: Heightfield,
    /// The outer wall, counterclockwise.
    pub outer: Vec<P>,
    /// Every straight run of stone wall.
    pub walls: Vec<Wall>,
    /// The inner walls as whole lines, gaps included, low z to high z.
    pub dividers: Vec<Vec<P>>,
    /// The gaps in the inner walls. The Fold's own is `fold.gap`.
    pub gaps: Vec<Gap>,
    pub fold: Fold,
    pub trees: Vec<Tree>,
    pub cairns: Vec<Cairn>,
    pub slabs: Vec<Slab>,
    pub tunnels: Vec<Tunnel>,
    /// The mounds out at the start of a match.
    pub mounds: Vec<Mound>,
    /// Spots for new mounds, in the order they come up.
    pub spare_mounds: Vec<Mound>,
    pub sheepdog_starts: Vec<P>,
    /// Mouths, the ones farthest from the Fold first.
    pub wolf_starts: Vec<P>,
    /// Where `sheep_starts` begins its own dice.
    #[serde(skip)]
    pub sheep_seed: u64,
    #[serde(skip)]
    pub solids: Solids,
}

impl Map {
    /// The Big map for this seed.
    pub fn generate(seed: u64) -> Map {
        Map::sized(seed, MapSize::Big)
    }

    /// The map for this seed and size. The same two give the same map, bit
    /// for bit, on every machine.
    pub fn sized(seed: u64, size: MapSize) -> Map {
        let mut root_rng = Rng::new(seed);
        let mut terrain_rng = Rng::new(root_rng.next_u64());
        let mut wall_rng = Rng::new(root_rng.next_u64());
        let mut prop_rng = Rng::new(root_rng.next_u64());
        let mut start_rng = Rng::new(root_rng.next_u64());
        let layout = walls::build(&mut wall_rng, size);
        let mut map = Map {
            seed,
            size,
            terrain: terrain::build(&mut terrain_rng, size),
            outer: layout.outer,
            walls: layout.walls,
            dividers: layout.dividers,
            gaps: layout.gaps,
            fold: layout.fold,
            trees: Vec::new(),
            cairns: Vec::new(),
            slabs: Vec::new(),
            tunnels: Vec::new(),
            mounds: Vec::new(),
            spare_mounds: Vec::new(),
            sheepdog_starts: Vec::new(),
            wolf_starts: Vec::new(),
            sheep_seed: root_rng.next_u64(),
            solids: Solids::default(),
        };
        map.rebuild_solids();
        props::place(&mut map, &mut prop_rng);
        map.rebuild_solids();
        map.sheepdog_starts = starts::sheepdog_starts(&mut start_rng, &map);
        map.wolf_starts = starts::wolf_starts(&map);
        map
    }

    /// From the low end to the Fold's end.
    pub fn length(&self) -> Fx {
        self.size.length()
    }

    /// From side to side.
    pub fn width(&self) -> Fx {
        self.size.width()
    }

    /// Every gap, the Fold's last.
    pub fn every_gap(&self) -> impl Iterator<Item = &Gap> {
        self.gaps.iter().chain(std::iter::once(&self.fold.gap))
    }

    /// Every tunnel mouth, each tunnel's two together.
    pub fn mouths(&self) -> Vec<P> {
        self.tunnels.iter().flat_map(|t| [t.a, t.b]).collect()
    }

    /// Start spots for a flock of `count`.
    pub fn sheep_starts(&self, count: usize) -> Vec<P> {
        starts::sheep_starts(self, count)
    }

    /// Rebuilds the collision cells from the walls and props.
    pub fn rebuild_solids(&mut self) {
        let mut list: Vec<Solid> = self
            .walls
            .iter()
            .map(|w| {
                let part = match w.kind {
                    WallKind::Outer => Part::Outer,
                    WallKind::Inner => Part::Inner,
                    WallKind::Fold => Part::FoldWall,
                };
                Solid::line(w.a, w.b, w.half, w.height, part)
            })
            .collect();
        let g = &self.fold.gap;
        list.push(Solid::line(g.a, g.b, table::WALL_HALF, 0, Part::FoldGap));
        list.extend(
            self.trees
                .iter()
                .map(|t| Solid::circle((t.x, t.z), t.trunk, Part::Trunk)),
        );
        list.extend(
            self.cairns
                .iter()
                .enumerate()
                .map(|(i, c)| Solid::circle((c.x, c.z), c.tiers[0].radius, Part::Cairn(i as u8))),
        );
        list.extend(
            self.slabs
                .iter()
                .map(|s| Solid::line(s.a, s.b, s.half, s.top, Part::Slab)),
        );
        self.solids = Solids::new(list, &self.outer, self.length(), self.width());
    }

    /// The height of the slab's top over (x, z), if one is there.
    fn slab_top(&self, x: Fx, z: Fx) -> Option<Fx> {
        self.solids.near(x, z).iter().find_map(|&id| {
            let s = &self.solids.list[id as usize];
            if s.part != Part::Slab {
                return None;
            }
            let (qx, qz) = s.nearest(x, z);
            let (dx, dz) = ((x - qx) as i64, (z - qz) as i64);
            (dx * dx + dz * dz < s.half as i64 * s.half as i64).then_some(s.top)
        })
    }

    /// The bare ground, without cairns or slabs.
    pub fn terrain_height(&self, x: Fx, z: Fx) -> Fx {
        self.terrain.at(x, z)
    }

    /// Is the point inside the outer wall's line?
    pub fn inside(&self, x: Fx, z: Fx) -> bool {
        polygon_contains(&self.outer, x, z)
    }

    /// Is the point in the Fold? Its front line counts as the gap.
    pub fn in_fold(&self, x: Fx, z: Fx) -> bool {
        polygon_contains(&self.fold.outline, x, z)
    }

    /// Which field holds the point, numbered from the low end: `None` in the
    /// Fold or outside the outer wall.
    pub fn field_at(&self, x: Fx, z: Fx) -> Option<usize> {
        if !self.inside(x, z) || self.in_fold(x, z) {
            return None;
        }
        Some(self.dividers.iter().filter(|line| x > line_x_at(line, z)).count())
    }

    /// Moves (x, z) back inside the outer wall: to the nearest point of the
    /// wall, then a body's width toward a point just below the Fold's gap.
    /// The whole outline can be seen from there, so that way is always in.
    fn pull_inside(&self, x: Fx, z: Fx, radius: Fx) -> P {
        let anchor = (self.fold.gap.center.0 - table::meters(8), self.fold.gap.center.1);
        let mut best = (i64::MAX, (x, z), 0);
        for s in self.solids.list.iter().filter(|s| s.part == Part::Outer) {
            let (qx, qz) = s.nearest(x, z);
            let (dx, dz) = ((x - qx) as i64, (z - qz) as i64);
            let d2 = dx * dx + dz * dz;
            if d2 < best.0 {
                best = (d2, (qx, qz), s.half);
            }
        }
        let (_, (qx, qz), half) = best;
        let (dx, dz) = ((anchor.0 - qx) as i64, (anchor.1 - qz) as i64);
        let d = root((dx * dx + dz * dz) as u64) as i64;
        let away = (half + radius) as i64 + SKIN;
        (qx + (dx * away / d) as Fx, qz + (dz * away / d) as Fx)
    }

    /// The half width this solid has for this mover where it is nearest
    /// them, or `None` when it lets them pass.
    fn half_for(&self, s: &Solid, q: P, at: P, mover: Mover) -> Option<Fx> {
        let cleared = || {
            mover
                .feet()
                .is_some_and(|f| f >= self.terrain.at(q.0, q.1) + s.top)
        };
        match s.part {
            Part::Outer | Part::Trunk => Some(s.half),
            Part::Cairn(i) => self.cairns[i as usize].stopping_radius(mover.feet()),
            Part::Inner => (!cleared()).then_some(s.half),
            // measured from the ground under the body, as `height` is, so a
            // body on the slab's uphill side can't walk onto it
            Part::Slab => {
                let top = self.terrain.at(at.0, at.1) + s.top;
                let over = mover.feet().is_some_and(|f| f + table::SLAB_STEP_UP >= top);
                (mover != Mover::Sight && !over).then_some(s.half)
            }
            Part::FoldWall => match mover {
                Mover::Wolf { .. } => Some(s.half),
                _ => (!cleared()).then_some(s.half),
            },
            Part::FoldGap => match mover {
                Mover::Wolf { .. } | Mover::FoldedSheep => Some(s.half),
                Mover::Sheep | Mover::Sight | Mover::Sheepdog { .. } => None,
            },
        }
    }

    /// Is the Fold what holds this Wolf back? True when a Wolf here stops
    /// where a Sheepdog at the same height would not, so the rules can tell
    /// the Wolf why.
    pub fn fold_stops_wolf(&self, x: Fx, z: Fx, radius: Fx, feet: Fx) -> bool {
        let wolf = self.push_out_as(x, z, radius, Mover::Wolf { feet });
        wolf != self.push_out_as(x, z, radius, Mover::Sheepdog { feet })
    }

    /// Pushes a circle out of whatever stops this mover; returns the new
    /// x, z. The answer is always inside the outer wall.
    pub fn push_out_as(&self, x: Fx, z: Fx, radius: Fx, mover: Mover) -> P {
        let edge = solids::OVERHANG;
        let mut x = x.clamp(-edge, self.length() + edge);
        let mut z = z.clamp(-edge, self.width() + edge);
        let out = match self.solids.zone(x, z) {
            Zone::Inside => false,
            Zone::Outside => true,
            Zone::Mixed => !self.inside(x, z),
        };
        if out {
            (x, z) = self.pull_inside(x, z, radius);
        }
        // a cell's list stops at REACH; a wider circle needs every solid
        let wide = radius > solids::REACH;
        let all: Vec<u16> = if wide {
            (0..self.solids.list.len() as u16).collect()
        } else {
            Vec::new()
        };
        for _ in 0..PUSH_ROUNDS {
            let ids: &[u16] = if wide { &all } else { self.solids.near(x, z) };
            let mut moved = false;
            for &id in ids {
                let s = &self.solids.list[id as usize];
                let (qx, qz) = s.nearest(x, z);
                let (dx, dz) = ((x - qx) as i64, (z - qz) as i64);
                let d2 = dx * dx + dz * dz;
                let widest = (s.half + radius) as i64;
                if d2 >= widest * widest {
                    continue;
                }
                let Some(half) = self.half_for(s, (qx, qz), (x, z), mover) else {
                    continue;
                };
                let reach = (half + radius) as i64;
                if d2 >= reach * reach {
                    continue;
                }
                let target = reach + SKIN;
                if d2 == 0 {
                    let (nx, nz) = s.left();
                    x = qx + ((nx as i64 * target) >> 16) as Fx;
                    z = qz + ((nz as i64 * target) >> 16) as Fx;
                } else {
                    let d = root(d2 as u64) as i64;
                    x = qx + (dx * target / d) as Fx;
                    z = qz + (dz * target / d) as Fx;
                }
                moved = true;
            }
            if !moved {
                break;
            }
        }
        (x, z)
    }
}

/// The x of a low-to-high line at z. Past its ends it holds the end's x.
fn line_x_at(line: &[P], z: Fx) -> Fx {
    for pair in line.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if z <= b.1 {
            if b.1 <= a.1 || z <= a.1 {
                return a.0;
            }
            return a.0 + ((b.0 - a.0) as i64 * (z - a.1) as i64 / (b.1 - a.1) as i64) as Fx;
        }
    }
    line[line.len() - 1].0
}

impl Ground for Map {
    /// The ground, or the top of a cairn's step or a slab where there is one.
    fn height(&self, x: Fx, z: Fx) -> Fx {
        let ground = self.terrain.at(x, z);
        match self.cairns.iter().find_map(|c| c.top_at(x, z)) {
            Some(top) => ground.max(top),
            None => ground + self.slab_top(x, z).unwrap_or(0),
        }
    }

    /// The contract's call, for a body with no jump: a sheep outside the
    /// Fold. Everything stops it and the Fold's gap is open. Sheepdogs and
    /// Wolves use `push_out_as`, which knows how high their feet are.
    fn push_out(&self, x: Fx, z: Fx, radius: Fx) -> (Fx, Fx) {
        self.push_out_as(x, z, radius, Mover::Sheep)
    }

    fn push_out_as(&self, x: Fx, z: Fx, radius: Fx, mover: Mover) -> (Fx, Fx) {
        Map::push_out_as(self, x, z, radius, mover)
    }

    /// A sheep is always inside the outer wall, so this skips the test of
    /// that in `field_at`, at a few thousand asks a second.
    fn field(&self, x: Fx, z: Fx) -> u8 {
        if self.in_fold(x, z) {
            return FOLD_FIELD;
        }
        self.dividers.iter().filter(|line| x > line_x_at(line, z)).count() as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::{div, from_int, length, mul, ONE};
    use table::meters;

    #[test]
    fn a_seed_keeps_the_size_of_the_map() {
        for seed in 0..50 {
            let m = Map::generate(seed);
            let xs = m.outer.iter().map(|p| p.0);
            let zs = m.outer.iter().map(|p| p.1);
            let (x_lo, x_hi) = (xs.clone().min().unwrap(), xs.max().unwrap());
            let (z_lo, z_hi) = (zs.clone().min().unwrap(), zs.max().unwrap());
            assert!((x_hi - x_lo - table::LENGTH).abs() <= meters(5), "seed {seed}");
            assert!((z_hi - z_lo - table::WIDTH).abs() <= meters(5), "seed {seed}");
        }
    }

    #[test]
    fn fields_are_numbered_up_the_hill() {
        let m = Map::generate(1);
        assert_eq!(m.field_at(from_int(40), from_int(88)), Some(0));
        assert_eq!(m.field_at(from_int(160), from_int(88)), Some(1));
        assert_eq!(m.field_at(from_int(255), from_int(88)), Some(2));
        assert_eq!(m.field_at(from_int(312), from_int(88)), None, "the Fold");
        assert_eq!(m.field_at(from_int(-30), from_int(88)), None, "past the wall");
        assert!(m.in_fold(from_int(312), from_int(88)));
    }

    #[test]
    fn every_inner_wall_has_one_or_two_gaps_of_the_right_width() {
        let mut counts = [0; 3];
        for seed in 0..100 {
            let m = Map::generate(seed);
            for wall in 0..2 {
                let n = m.gaps.iter().filter(|g| g.wall == wall).count();
                assert!(n == 1 || n == 2, "seed {seed} wall {wall} has {n} gaps");
                counts[n] += 1;
            }
            for g in &m.gaps {
                assert!(
                    g.width >= meters(6) && g.width <= meters(10),
                    "seed {seed}: {}",
                    g.width
                );
                let measured = length(g.b.0 - g.a.0, g.b.1 - g.a.1);
                assert!((measured - g.width).abs() < table::tenths(1), "seed {seed}");
            }
        }
        assert!(counts[1] > 40 && counts[2] > 40, "{counts:?}");
    }

    #[test]
    fn terrain_rises_toward_the_fold() {
        for seed in 0..50 {
            let m = Map::generate(seed);
            let band = |x: i32| -> i64 {
                (0..17)
                    .map(|i| m.terrain_height(from_int(x), from_int(5 + 10 * i)) as i64)
                    .sum()
            };
            assert!(band(25) < band(150) && band(150) < band(300), "seed {seed}");
        }
    }

    #[test]
    fn a_circle_is_pushed_clear_of_a_straight_wall() {
        let m = Map::generate(3);
        let wall = m.walls.iter().find(|w| w.kind == WallKind::Inner).unwrap();
        let mid = ((wall.a.0 + wall.b.0) / 2, (wall.a.1 + wall.b.1) / 2);
        let (x, z) = m.push_out(mid.0 + table::tenths(2), mid.1, table::tenths(4));
        let s = Solid::line(wall.a, wall.b, wall.half, wall.height, Part::Inner);
        let (qx, qz) = s.nearest(x, z);
        // half the wall (0.5) plus the radius (0.4) from the wall's line
        let got = length(x - qx, z - qz);
        assert!((got - table::tenths(9)).abs() < 40, "{got}");
    }

    #[test]
    fn a_body_outside_the_outer_wall_comes_back_inside() {
        let m = Map::generate(4);
        for (x, z) in [(-30, 88), (160, -40), (160, 235), (355, 12), (30000, -30000)] {
            let (nx, nz) = m.push_out(from_int(x), from_int(z), table::tenths(4));
            assert!(m.inside(nx, nz), "({x}, {z})");
        }
    }

    #[test]
    fn a_jump_clears_an_inner_wall_but_never_the_outer_wall() {
        let m = Map::generate(5);
        let r = table::tenths(4);
        let wall = m.walls.iter().find(|w| w.kind == WallKind::Inner).unwrap();
        let mid = ((wall.a.0 + wall.b.0) / 2, (wall.a.1 + wall.b.1) / 2);
        let ground = m.terrain_height(mid.0, mid.1);
        let top = ground + wall.height + table::tenths(1);
        for mover in [Mover::Sheepdog { feet: top }, Mover::Wolf { feet: top }] {
            assert_eq!(m.push_out_as(mid.0, mid.1, r, mover), mid, "clears the top");
        }
        let low = Mover::Sheepdog { feet: ground };
        assert_ne!(m.push_out_as(mid.0, mid.1, r, low), mid, "too low");
        assert_ne!(m.push_out(mid.0, mid.1, r), mid, "sheep don't jump");

        let high = Mover::Sheepdog { feet: from_int(1000) };
        // just inside the wall's line, where only the barrier can act: on the
        // line itself the body would be pulled back in whatever the barrier did
        let outer = m.walls.iter().find(|w| w.kind == WallKind::Outer).unwrap();
        let mid = ((outer.a.0 + outer.b.0) / 2, (outer.a.1 + outer.b.1) / 2);
        let (cx, cz) = (table::LENGTH / 2 - mid.0, table::WIDTH / 2 - mid.1);
        let n = length(cx, cz);
        let near = table::tenths(2);
        let at = (mid.0 + mul(div(cx, n), near), mid.1 + mul(div(cz, n), near));
        assert!(m.inside(at.0, at.1));
        assert_ne!(
            m.push_out_as(at.0, at.1, r, high),
            at,
            "no height clears the outer wall"
        );
    }

    #[test]
    fn a_slab_stops_whatever_walks_and_a_jump_stands_on_it() {
        let m = Map::generate(3);
        let r = table::tenths(4);
        for s in &m.slabs {
            let (way_x, way_z) = (s.b.0 - s.a.0, s.b.1 - s.a.1);
            let long = length(way_x, way_z);
            let side = (div(-way_z, long), div(way_x, long));
            // a body touching the side, a meter along, and on the end's far tip
            let inward = s.half + r - table::tenths(1);
            let along = (
                s.a.0 + mul(div(way_x, long), ONE),
                s.a.1 + mul(div(way_z, long), ONE),
            );
            let beside = (along.0 + mul(side.0, inward), along.1 + mul(side.1, inward));
            let across = (along.0 - mul(side.0, inward), along.1 - mul(side.1, inward));
            for at in [beside, across, s.a, s.b, s.center()] {
                let ground = m.terrain_height(at.0, at.1);
                for mover in [
                    Mover::Sheep,
                    Mover::FoldedSheep,
                    Mover::Sheepdog { feet: ground },
                    Mover::Wolf { feet: ground },
                ] {
                    let (x, z) = m.push_out_as(at.0, at.1, r, mover);
                    let line = Solid::line(s.a, s.b, s.half, s.top, Part::Slab);
                    let (qx, qz) = line.nearest(x, z);
                    let d = length(x - qx, z - qz);
                    assert!((d - s.half - r).abs() < 40, "{mover:?}: {d}");
                }
                // feet at the top pass, and stand there
                let top = ground + s.top;
                for mover in [Mover::Sheepdog { feet: top }, Mover::Wolf { feet: top }] {
                    assert_eq!(m.push_out_as(at.0, at.1, r, mover), at);
                }
                if at != beside && at != across {
                    assert_eq!(m.height(at.0, at.1), top);
                }
                // a whisker under the step up does not
                let low = Mover::Sheepdog {
                    feet: top - table::SLAB_STEP_UP - 1,
                };
                assert_ne!(m.push_out_as(at.0, at.1, r, low), at);
            }
        }
    }

    #[test]
    fn the_eye_looks_over_a_slab_but_not_through_a_wall() {
        let m = Map::generate(3);
        let thin = ONE / 10;
        let c = m.slabs[0].center();
        assert_eq!(m.push_out_as(c.0, c.1, thin, Mover::Sight), c);
        assert_ne!(m.push_out_as(c.0, c.1, thin, Mover::Sheep), c);
        let w = m.walls.iter().find(|w| w.kind == WallKind::Inner).unwrap();
        let mid = ((w.a.0 + w.b.0) / 2, (w.a.1 + w.b.1) / 2);
        assert_ne!(m.push_out_as(mid.0, mid.1, thin, Mover::Sight), mid);
    }

    #[test]
    fn the_fold_says_when_it_stopped_a_wolf() {
        let m = Map::generate(7);
        let r = table::tenths(4);
        let g = m.fold.gap.center;
        assert!(m.fold_stops_wolf(g.0, g.1, r, m.terrain_height(g.0, g.1)));
        // an inner wall stops both alike, so that is no business of the Fold's
        let w = m.walls.iter().find(|w| w.kind == WallKind::Inner).unwrap();
        let mid = ((w.a.0 + w.b.0) / 2, (w.a.1 + w.b.1) / 2);
        assert!(!m.fold_stops_wolf(mid.0, mid.1, r, m.terrain_height(mid.0, mid.1)));
    }

    #[test]
    fn the_fold_keeps_wolves_out_and_folded_sheep_in() {
        let m = Map::generate(6);
        let c = m.fold.gap.center;
        let r = table::tenths(4);
        let high = from_int(1000);
        // open to a sheep outside and to a Sheepdog
        assert_eq!(m.push_out_as(c.0, c.1, r, Mover::Sheep), c);
        assert_eq!(m.push_out_as(c.0, c.1, r, Mover::Sheepdog { feet: 0 }), c);
        // shut to a Wolf, even in the air, and to a folded sheep
        assert_ne!(m.push_out_as(c.0, c.1, r, Mover::Wolf { feet: high }), c);
        assert_ne!(m.push_out_as(c.0, c.1, r, Mover::FoldedSheep), c);
        // the front wall stops a Wolf at any height and lets a Sheepdog jump
        let w = m.walls.iter().find(|w| w.kind == WallKind::Fold).unwrap();
        let mid = ((w.a.0 + w.b.0) / 2, (w.a.1 + w.b.1) / 2);
        assert_ne!(m.push_out_as(mid.0, mid.1, r, Mover::Wolf { feet: high }), mid);
        assert_eq!(
            m.push_out_as(mid.0, mid.1, r, Mover::Sheepdog { feet: high }),
            mid
        );
    }
}
