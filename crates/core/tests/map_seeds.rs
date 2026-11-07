//! Every seed is playable. These run the whole map over a few hundred seeds
//! and measure the result through the same calls bodies use, so a number in
//! `map::table` can't vouch for itself.

use tool_core::fx::{self, from_int, Fx, ONE};
use tool_core::ground::Ground;
use tool_core::map::proof::{Flood, GRID_SLACK};
use tool_core::map::table::{self, meters, tenths};
use tool_core::map::walls::{Wall, WallKind};
use tool_core::map::{Map, MapSize, Mover, P};

/// How many seeds the sweeps cover.
const SEEDS: u64 = 300;

/// The width each sweep proves: gaps start at 6 m, and the 0.5 m grid can
/// only see a channel at least 0.71 m wide at any angle, so it floods at a
/// radius of 2.6 m. See `map::proof`.
const FLOOD_RADIUS: Fx = meters(26) / 10;
/// A sheep's body, and a Wolf's.
const SHEEP_RADIUS: Fx = meters(6) / 10;

/// Every seed at every Map size. What comes with each map numbers it for the
/// messages: its seed, plus 1000 on Medium and 2000 on Big.
fn each_map() -> impl Iterator<Item = (u64, Map)> {
    MapSize::ALL
        .into_iter()
        .enumerate()
        .flat_map(|(k, size)| (0..SEEDS).map(move |seed| (seed + 1000 * k as u64, Map::sized(seed, size))))
}

fn sweep(mut check: impl FnMut(u64, &Map)) {
    for (seed, m) in each_map() {
        check(seed, &m);
    }
}

fn dist(a: P, b: P) -> Fx {
    fx::length(a.0 - b.0, a.1 - b.1)
}

/// How far `p` lies from the wall's line.
fn wall_distance(w: &Wall, p: P) -> Fx {
    line_distance(w.a, w.b, p)
}

/// How far `p` lies from the line from `a` to `b`.
fn line_distance(a: P, b: P, p: P) -> Fx {
    let (ex, ez) = ((b.0 - a.0) >> 4, (b.1 - a.1) >> 4);
    let (px, pz) = ((p.0 - a.0) >> 4, (p.1 - a.1) >> 4);
    let len2 = ex as i64 * ex as i64 + ez as i64 * ez as i64;
    let dot = px as i64 * ex as i64 + pz as i64 * ez as i64;
    let t = if len2 == 0 {
        0
    } else {
        ((dot << 16) / len2).clamp(0, 65536)
    };
    let q = (
        a.0 + (((b.0 - a.0) as i64 * t) >> 16) as Fx,
        a.1 + (((b.1 - a.1) as i64 * t) >> 16) as Fx,
    );
    dist(p, q)
}

/// The props that take up room: their centers and surface radii.
fn props(m: &Map) -> Vec<(P, Fx)> {
    let trees = m.trees.iter().map(|t| ((t.x, t.z), t.trunk));
    let cairns = m.cairns.iter().map(|c| ((c.x, c.z), c.tiers[0].radius));
    let mouths = m.mouths().into_iter().map(|p| (p, 0));
    let mounds = m.mounds.iter().chain(&m.spare_mounds).map(|d| ((d.x, d.z), 0));
    trees.chain(cairns).chain(mouths).chain(mounds).collect()
}

/// Those, and the slabs as circles round them.
fn props_and_slabs(m: &Map) -> Vec<(P, Fx)> {
    let slabs = m.slabs.iter().map(|s| (s.center(), s.reach()));
    props(m).into_iter().chain(slabs).collect()
}

#[test]
fn the_same_seed_gives_the_same_map_and_another_seed_a_different_one() {
    for seed in [0, 1, 7, 12345, u64::MAX] {
        assert_eq!(Map::generate(seed), Map::generate(seed), "seed {seed}");
        assert_eq!(
            serde_json::to_string(&Map::generate(seed)).unwrap(),
            serde_json::to_string(&Map::generate(seed)).unwrap()
        );
    }
    let (a, b) = (Map::generate(1), Map::generate(2));
    assert_ne!(a.trees, b.trees);
    assert_ne!(a.tunnels, b.tunnels);
    assert_ne!(a.gaps, b.gaps);
    assert_ne!(a.terrain, b.terrain);
}

#[test]
fn every_seed_has_one_open_ground_from_the_fold_to_every_field() {
    sweep(|seed, m| {
        let wide = Flood::from(m, m.fold.center, FLOOD_RADIUS, Mover::Sheep);
        let places = wide.reached_by_place(m);
        assert!(places.iter().all(|&n| n > 500), "seed {seed}: {places:?}");
        assert_eq!(wide.sealed(), 0, "seed {seed}: ground wide enough but cut off");
    });
}

#[test]
fn no_sheep_sized_pocket_is_cut_off_from_the_fold() {
    sweep(|seed, m| {
        let flood = Flood::from(m, m.fold.center, SHEEP_RADIUS, Mover::Sheep);
        assert_eq!(flood.sealed(), 0, "seed {seed}");
    });
}

#[test]
fn every_gap_measures_its_stated_width_and_stays_open() {
    sweep(|seed, m| {
        for g in m.every_gap() {
            let w = g.width;
            assert!((meters(6)..=meters(10)).contains(&w), "seed {seed}: {w}");
            // a circle just narrower than the gap fits at its center, and one
            // just wider does not
            let fits = w / 2 - ONE / 20;
            let jams = w / 2 + ONE / 20;
            let c = g.center;
            assert_eq!(m.push_out_as(c.0, c.1, fits, Mover::Sheep), c, "seed {seed}");
            assert_ne!(m.push_out_as(c.0, c.1, jams, Mover::Sheep), c, "seed {seed}");
        }
    });
}

/// The same map with a stone line across every gap, the Fold's too.
fn with_gaps_closed(m: &Map) -> Map {
    let mut closed = m.clone();
    for g in m.every_gap() {
        closed.walls.push(Wall {
            a: g.a,
            b: g.b,
            half: table::WALL_HALF,
            height: table::INNER_WALL_HEIGHT,
            kind: WallKind::Outer,
        });
    }
    closed.rebuild_solids();
    closed
}

#[test]
fn sheep_can_only_change_fields_through_a_gap() {
    for (seed, m) in each_map() {
        let m = with_gaps_closed(&m);
        let small = tenths(3);
        // a cairn can stand on any fixed spot, so start clear of it
        let start = m.push_out(from_int(40), from_int(70), ONE);
        let places = Flood::from(&m, start, small, Mover::Sheep);
        let got = places.reached_by_place(&m);
        assert!(got[0] > 1000, "seed {seed}: {got:?}");
        assert_eq!(got[1..], [0, 0, 0], "seed {seed}: the walls leak");
        let fold = Flood::from(&m, m.fold.center, small, Mover::Sheep);
        let got = fold.reached_by_place(&m);
        assert_eq!(got[..3], [0, 0, 0], "seed {seed}: the Fold leaks");
    }
}

#[test]
fn no_wolf_reaches_the_fold_at_any_height() {
    for (seed, m) in each_map() {
        for feet in [0, from_int(100)] {
            let wolf = Mover::Wolf { feet };
            let start = m.push_out(from_int(30), m.width() / 2, ONE);
            let flood = Flood::from(&m, start, tenths(3), wolf);
            let got = flood.reached_by_place(&m);
            assert!(
                got[0] > 0 && got[1] > 0 && got[2] > 0,
                "seed {seed} feet {feet}: {got:?}"
            );
            assert_eq!(got[3], 0, "seed {seed} feet {feet}: a Wolf got in");
        }
    }
}

#[test]
fn no_prop_sits_in_a_gap_in_the_fold_or_on_a_wall() {
    sweep(|seed, m| {
        for (p, radius) in props(m) {
            assert!(
                m.field_at(p.0, p.1).is_some(),
                "seed {seed}: {p:?} is outside the fields"
            );
            for g in m.every_gap() {
                assert!(dist(g.center, p) >= g.width, "seed {seed}: {p:?} crowds a gap");
            }
            for w in &m.walls {
                let clear = wall_distance(w, p) - radius - w.half;
                assert!(clear >= meters(2), "seed {seed}: {p:?} is on a wall");
            }
        }
    });
}

#[test]
fn trees_cairns_and_slabs_leave_a_route_wider_than_a_gap_around_them() {
    sweep(|seed, m| {
        let solid: Vec<(P, Fx)> = props_and_slabs(m).into_iter().filter(|&(_, r)| r > 0).collect();
        for (i, &(p, r)) in solid.iter().enumerate() {
            for &(q, s) in &solid[i + 1..] {
                assert!(dist(p, q) - r - s >= meters(10), "seed {seed}: {p:?} and {q:?}");
            }
            for w in &m.walls {
                let clear = wall_distance(w, p) - r - w.half;
                assert!(clear >= meters(6), "seed {seed}: {p:?} is {clear} from a wall");
            }
        }
    });
}

#[test]
fn trees_and_cairns_keep_out_of_the_wedges_where_walls_meet() {
    // 18 m out, a Sheepdog at the back of the trunk is about 16 m from the
    // wall's end and sheep in the wedge at least 3 m from it, so 13 m apart,
    // past the 9 m flight radius: it can't hold them there by standing still
    sweep(|seed, m| {
        let spots = m.trees.iter().map(|t| (t.x, t.z));
        for p in spots.chain(m.cairns.iter().map(|c| (c.x, c.z))) {
            for line in &m.dividers {
                for end in [line[0], line[line.len() - 1]] {
                    assert!(dist(p, end) >= meters(18), "seed {seed}: {p:?} by {end:?}");
                }
            }
        }
    });
}

#[test]
fn no_prop_sits_on_another_prop() {
    sweep(|seed, m| {
        let all = props_and_slabs(m);
        for (i, &(p, r)) in all.iter().enumerate() {
            for &(q, s) in &all[i + 1..] {
                assert!(dist(p, q) >= r + s + ONE, "seed {seed}: {p:?} on {q:?}");
            }
        }
    });
}

#[test]
fn slabs_keep_clear_of_all_a_body_or_the_flock_needs() {
    let mut most_and_least = [(0, usize::MAX); 3];
    sweep(|seed, m| {
        let tally = &mut most_and_least[m.size as usize];
        *tally = (tally.0.max(m.slabs.len()), tally.1.min(m.slabs.len()));
        let spots: Vec<P> = m
            .mouths()
            .into_iter()
            .chain(m.mounds.iter().chain(&m.spare_mounds).map(|d| (d.x, d.z)))
            .chain(m.sheepdog_starts.iter().copied())
            .collect();
        let sheep = m.sheep_starts(100);
        for s in &m.slabs {
            let off = |p: P| line_distance(s.a, s.b, p) - s.half;
            assert!(m.field_at(s.a.0, s.a.1).is_some(), "seed {seed}: {s:?}");
            assert!(m.field_at(s.b.0, s.b.1).is_some(), "seed {seed}: {s:?}");
            for w in &m.walls {
                let near = wall_distance(w, s.a).min(wall_distance(w, s.b));
                assert!(near - s.half - w.half >= meters(6), "seed {seed}: {s:?}");
            }
            for g in m.every_gap() {
                assert!(off(g.center) >= meters(18), "seed {seed}: {s:?}");
            }
            for &p in &spots {
                assert!(off(p) >= meters(4), "seed {seed}: {s:?} by {p:?}");
            }
            for &p in &sheep {
                assert!(off(p) >= SHEEP_RADIUS, "seed {seed}: {s:?} by {p:?}");
            }
        }
    });
    println!("slabs on each size, most and least: {most_and_least:?}");
    assert!(most_and_least.iter().all(|t| t.0 >= 4), "{most_and_least:?}");
}

#[test]
fn tunnels_are_fixed_pairs_that_span_real_distance() {
    sweep(|seed, m| {
        assert_eq!(m.tunnels.len(), 4, "seed {seed}");
        for t in &m.tunnels {
            // 112 m on Big, and in step with the hill below the Fold: 299 m
            // of it on Big, 218 m on Medium, 169 m on Small
            let least = match m.size {
                MapSize::Small => meters(63),
                MapSize::Medium => meters(81),
                MapSize::Big => meters(112),
            };
            assert!(dist(t.a, t.b) >= least, "seed {seed}: {t:?}");
            assert_eq!(t.other(t.a), t.b);
            assert_eq!(t.other(t.b), t.a);
            let fields = (m.field_at(t.a.0, t.a.1), m.field_at(t.b.0, t.b.1));
            assert_ne!(fields.0, fields.1, "seed {seed}: {t:?} stays in one field");
        }
    });
}

#[test]
fn tunnel_mouths_are_spread_out_and_in_every_field() {
    sweep(|seed, m| {
        let mouths = m.mouths();
        assert_eq!(mouths.len(), 8, "seed {seed}");
        for (i, &a) in mouths.iter().enumerate() {
            for &b in &mouths[i + 1..] {
                // 50 m, eased at most two rounds to 37.5 m on Big and on
                // Medium, in step with the hill below the Fold, and three
                // rounds on Small: 50 by 169/299 by 5/8 is 17.7 m
                let least = match m.size {
                    MapSize::Small => tenths(170),
                    MapSize::Medium => tenths(270),
                    MapSize::Big => tenths(375),
                };
                assert!(dist(a, b) >= least, "seed {seed}: {a:?} and {b:?}");
            }
            assert!(dist(a, m.fold.gap.center) >= meters(30), "seed {seed}: {a:?}");
        }
        for field in 0..3 {
            let n = mouths
                .iter()
                .filter(|p| m.field_at(p.0, p.1) == Some(field))
                .count();
            assert!(n >= 2, "seed {seed}: field {field} has {n} mouths");
        }
    });
}

#[test]
fn the_right_number_of_mounds_sit_at_the_base_of_a_tree_or_cairn() {
    sweep(|seed, m| {
        assert_eq!(m.mounds.len(), 16, "seed {seed}");
        assert_eq!(m.spare_mounds.len(), 16, "seed {seed}");
        let anchors: Vec<(P, Fx)> = props(m).into_iter().filter(|&(_, r)| r > 0).collect();
        // lone trees and cairns, and young groups of 2 or 3 trees, short of
        // a tree where a group ran out of room
        let (lone, groups) = match m.size {
            MapSize::Small => (9 + 3, 2),
            MapSize::Medium => (11 + 6, 4),
            MapSize::Big => (16 + 9, 5),
        };
        let some = lone + groups..=lone + 3 * groups;
        assert!(some.contains(&anchors.len()), "seed {seed}");
        let mut per_anchor = vec![0; anchors.len()];
        for d in m.mounds.iter().chain(&m.spare_mounds) {
            let p = (d.x, d.z);
            let found = anchors.iter().position(|&(c, r)| dist(c, p) <= r + tenths(15));
            let Some(i) = found else {
                panic!("seed {seed}: mound {p:?} is at the base of nothing");
            };
            per_anchor[i] += 1;
            // room to stand on it
            let body = tenths(5);
            assert_eq!(m.push_out(p.0, p.1, body), p, "seed {seed}: {p:?}");
        }
        assert!(per_anchor.iter().all(|&n| n <= 2), "seed {seed}: {per_anchor:?}");
    });
}

#[test]
fn every_field_holds_groups_of_young_trees() {
    // the first of each group always stands, and a group of young trees has
    // a trunk of at most 0.4 m, under a lone tree's 0.6 m
    sweep(|seed, m| {
        let young = |field: usize| {
            let in_field = |t: &&tool_core::map::props::Tree| m.field_at(t.x, t.z) == Some(field);
            let small = |t: &&tool_core::map::props::Tree| t.trunk <= tenths(4);
            m.trees.iter().filter(in_field).filter(small).count()
        };
        let least = match m.size {
            MapSize::Small => [1, 1, 0],
            MapSize::Medium => [2, 1, 1],
            MapSize::Big => [2, 2, 1],
        };
        for (field, &want) in least.iter().enumerate() {
            assert!(young(field) >= want, "seed {seed}: field {field}");
        }
        let (all, firsts) = ((0..3).map(young).sum::<usize>(), least.iter().sum());
        assert!(all > firsts, "seed {seed}: every group stayed a lone tree");
    });
}

#[test]
fn every_field_gets_mounds_at_the_start_and_later() {
    sweep(|seed, m| {
        for field in 0..3 {
            let at_start = m
                .mounds
                .iter()
                .filter(|d| m.field_at(d.x, d.z) == Some(field))
                .count();
            let later = m
                .spare_mounds
                .iter()
                .filter(|d| m.field_at(d.x, d.z) == Some(field))
                .count();
            assert!(at_start >= 3, "seed {seed}: field {field} starts with {at_start}");
            assert!(later >= 2, "seed {seed}: field {field} gets {later} later");
        }
    });
}

#[test]
fn every_start_mound_and_mouth_can_be_reached_on_foot() {
    sweep(|seed, m| {
        let body = tenths(5);
        let feet = Mover::Sheepdog { feet: 0 };
        let flood = Flood::from(m, m.fold.center, body, feet);
        for d in m.mounds.iter().chain(&m.spare_mounds) {
            assert!(flood.reaches((d.x, d.z)), "seed {seed}: mound ({}, {})", d.x, d.z);
        }
        for p in m.mouths().into_iter().chain(m.sheepdog_starts.iter().copied()) {
            assert!(flood.reaches(p), "seed {seed}: {p:?}");
        }
    });
}

#[test]
fn sheepdogs_start_by_the_fold_and_wolves_at_mouths() {
    sweep(|seed, m| {
        assert_eq!(m.sheepdog_starts.len(), 8, "seed {seed}");
        for (i, &p) in m.sheepdog_starts.iter().enumerate() {
            assert!(dist(p, m.fold.gap.center) <= meters(15), "seed {seed}");
            assert_eq!(m.field_at(p.0, p.1), Some(2), "seed {seed}: {p:?}");
            assert_eq!(m.push_out(p.0, p.1, ONE), p, "seed {seed}: {p:?}");
            for &q in &m.sheepdog_starts[i + 1..] {
                assert!(dist(p, q) >= tenths(25), "seed {seed}");
            }
        }
        assert!(m.wolf_starts.len() >= 7, "seed {seed}");
        let mouths = m.mouths();
        assert!(m.wolf_starts.iter().all(|p| mouths.contains(p)), "seed {seed}");
        let gap = m.fold.gap.center;
        let far = m
            .wolf_starts
            .windows(2)
            .all(|w| dist(w[0], gap) >= dist(w[1], gap));
        assert!(far, "seed {seed}: the farthest mouths come first");
    });
}

/// Sizes of the groups of sheep, where two sheep within 3 m share a group.
fn group_sizes(spots: &[P]) -> Vec<usize> {
    let mut group: Vec<usize> = (0..spots.len()).collect();
    fn find(group: &mut [usize], i: usize) -> usize {
        let mut i = i;
        while group[i] != i {
            group[i] = group[group[i]];
            i = group[i];
        }
        i
    }
    for i in 0..spots.len() {
        for j in i + 1..spots.len() {
            if dist(spots[i], spots[j]) < meters(3) {
                let (a, b) = (find(&mut group, i), find(&mut group, j));
                group[a] = b;
            }
        }
    }
    let mut sizes = vec![0; spots.len()];
    for i in 0..spots.len() {
        sizes[find(&mut group, i)] += 1;
    }
    sizes.into_iter().filter(|&n| n > 0).collect()
}

#[test]
fn sheep_start_in_large_clumps_in_every_field_for_any_flock_size() {
    for (seed, m) in each_map() {
        // Small has no room for 400 sheep in clumps set apart
        let flocks: &[usize] = if m.size == MapSize::Small {
            &[100, 150, 250]
        } else {
            &[100, 150, 250, 400]
        };
        for &count in flocks {
            let spots = m.sheep_starts(count);
            assert_eq!(spots.len(), count, "seed {seed}");
            let sizes = group_sizes(&spots);
            let biggest = sizes.iter().max().copied().unwrap_or(0);
            // a clump is at most 24 and the last in a field takes up to 7
            // left over, and two clumps that crossed would count as one
            assert!(biggest <= 31, "seed {seed}, {count} sheep: a clump of {biggest}");
            assert!(biggest >= 10, "seed {seed}, {count} sheep: no big clump");
            // the mean clump is 13; walls, props and a small map trim a few
            assert!(
                sizes.len() * 7 <= count,
                "seed {seed}, {count} sheep: {} clumps",
                sizes.len()
            );
            let alone = sizes.iter().filter(|&&n| n == 1).count();
            assert!(alone * 20 <= count, "seed {seed}, {count} sheep: {alone} alone");
            for field in 0..3 {
                let n = spots
                    .iter()
                    .filter(|p| m.field_at(p.0, p.1) == Some(field))
                    .count();
                assert!(n * 100 >= count * 15, "seed {seed}: field {field} has {n}");
            }
        }
    }
}

#[test]
fn a_flock_of_any_size_gets_exactly_that_many_spots() {
    let m = Map::generate(13);
    for count in [0, 1, 2, 5, 17, 99, 401, 700] {
        assert_eq!(m.sheep_starts(count).len(), count);
    }
}

#[test]
fn no_sheep_starts_in_the_fold_or_touching_anything() {
    for (seed, m) in each_map() {
        let spots = m.sheep_starts(400);
        for (i, &p) in spots.iter().enumerate() {
            assert!(!m.in_fold(p.0, p.1), "seed {seed}");
            assert!(m.field_at(p.0, p.1).is_some(), "seed {seed}");
            assert_eq!(m.push_out(p.0, p.1, ONE), p, "seed {seed}: {p:?}");
            for &q in &spots[i + 1..] {
                assert!(dist(p, q) >= tenths(9), "seed {seed}: sheep on sheep at {p:?}");
            }
        }
    }
}

#[test]
fn sheep_starts_repeat_for_the_same_map_and_flock() {
    let m = Map::generate(11);
    assert_eq!(m.sheep_starts(250), m.sheep_starts(250));
    // the very first spots differ on another seed, not only what follows them
    let other = Map::generate(12).sheep_starts(250);
    assert_ne!(m.sheep_starts(250)[..3], other[..3]);
}

#[test]
fn a_pushed_body_is_clear_and_stays_put() {
    sweep(|seed, m| {
        // a coarse walk over the whole map and past its edges
        let radius = tenths(5);
        for mover in [Mover::Sheep, Mover::Wolf { feet: 0 }] {
            for ix in (-20..375).step_by(7) {
                for iz in (-20..215).step_by(7) {
                    let (x, z) = (from_int(ix) + ONE / 3, from_int(iz) + ONE / 7);
                    let (nx, nz) = m.push_out_as(x, z, radius, mover);
                    assert!(m.inside(nx, nz), "seed {seed}: ({ix}, {iz})");
                    let again = m.push_out_as(nx, nz, radius, mover);
                    assert_eq!(again, (nx, nz), "seed {seed}: ({ix}, {iz}) moved twice");
                }
            }
        }
    });
}

#[test]
fn a_step_along_a_wall_never_slips_through_it() {
    // walk a fast body straight at each wall and see it held on its side
    for (seed, m) in each_map() {
        let radius = tenths(5);
        for w in m.walls.iter().filter(|w| w.kind == WallKind::Inner) {
            let mid = ((w.a.0 + w.b.0) / 2, (w.a.1 + w.b.1) / 2);
            for side in [-1, 1] {
                let start = m.push_out(mid.0 + side * meters(4), mid.1, radius);
                let (mut x, mut z) = start;
                let step = -side * (ONE / 5);
                for _ in 0..80 {
                    (x, z) = m.push_out(x + step, z, radius);
                }
                assert_eq!((x > mid.0), side > 0, "seed {seed}: through the wall at {mid:?}");
            }
        }
    }
}

#[test]
fn a_stepped_cairn_can_be_climbed_a_jump_at_a_time_and_only_so() {
    sweep(|seed, m| {
        let body = tenths(4);
        for c in &m.cairns {
            let center = (c.x, c.z);
            let at_edge = |radius: Fx| (c.x + radius, c.z);
            // sheep stop at the base; so does a Sheepdog on the ground
            let ground = m.terrain_height(c.x, c.z);
            assert_ne!(m.push_out(c.x, c.z, body), center, "seed {seed}");
            let walking = Mover::Sheepdog { feet: ground };
            let stopped = m.push_out_as(c.x, c.z, body, walking);
            assert!(dist(stopped, center) >= c.tiers[0].radius + body, "seed {seed}");
            // each step, a jump higher: the step holds the feet and the next
            // one stops them
            let mut feet = ground;
            for (i, t) in c.tiers.iter().enumerate() {
                assert!(t.top - feet <= ONE, "seed {seed}: a step too tall to jump");
                feet = t.top;
                let mover = Mover::Sheepdog { feet };
                let stand_at = c.tiers.get(i + 1).map_or(0, |n| n.radius + body + ONE / 16);
                let p = at_edge(stand_at);
                assert_eq!(m.push_out_as(p.0, p.1, body, mover), p, "seed {seed}: tier {i}");
                assert_eq!(
                    m.height(p.0, p.1),
                    t.top.max(m.terrain_height(p.0, p.1)),
                    "seed {seed}"
                );
                if let Some(next) = c.tiers.get(i + 1) {
                    let inside = at_edge(next.radius - ONE / 8);
                    assert_ne!(
                        m.push_out_as(inside.0, inside.1, body, mover),
                        inside,
                        "seed {seed}"
                    );
                }
            }
            // the top holds a Wolf, the widest body, standing at its center
            assert!(c.tiers[c.tiers.len() - 1].radius >= tenths(7), "seed {seed}");
        }
    });
}

#[test]
fn the_ground_is_gently_rolling_with_no_crease_or_cliff() {
    sweep(|seed, m| {
        let h = |x: Fx, z: Fx| m.terrain_height(x, z);
        let step = ONE / 2;
        let mut x = -meters(30);
        while x < table::LENGTH + meters(30) {
            let mut z = -meters(30);
            while z < table::WIDTH + meters(30) {
                let along = h(x + step, z) - h(x, z);
                let across = h(x, z + step) - h(x, z);
                // no slope over 1 in 3.5 on the half meter
                let most = step * 2 / 7;
                assert!(along.abs() <= most, "seed {seed} at ({x}, {z}): {along}");
                assert!(across.abs() <= most, "seed {seed} at ({x}, {z}): {across}");
                // the slope barely changes from one half meter to the next
                let bend = h(x + 2 * step, z) - 2 * h(x + step, z) + h(x, z);
                assert!(bend.abs() <= step / 20, "seed {seed} at ({x}, {z}): {bend}");
                z += meters(3);
            }
            x += meters(3);
        }
    });
}

#[test]
fn the_hill_climbs_to_the_fold_and_the_fold_sits_level() {
    sweep(|seed, m| {
        let mean = |x: i32| -> i64 {
            let sum: i64 = (1..17)
                .map(|i| m.terrain_height(from_int(x), from_int(i * 10)) as i64)
                .sum();
            sum / 16
        };
        assert!(mean(38) < mean(138), "seed {seed}");
        assert!(mean(138) < mean(250), "seed {seed}");
        // the ramp gives 30 * 212 / 325 = 19.6 m; the two octaves can take
        // 2.6 m off at each end, and a column's mean takes far less
        assert!(mean(250) - mean(38) >= meters(14) as i64, "seed {seed}");
        // across the Fold, rolling is down to a quarter
        let (lo, hi) = (from_int(302), from_int(318));
        for z in (70..106).step_by(5) {
            let rise = m.terrain_height(hi, from_int(z)) - m.terrain_height(lo, from_int(z));
            assert!(rise.abs() <= meters(3), "seed {seed}: {rise}");
        }
    });
}

#[test]
fn every_seed_keeps_the_same_rough_outline() {
    for seed in 0..SEEDS {
        let m = Map::generate(seed);
        assert_eq!(m.outer.len(), table::OUTER_OUTLINE.len(), "seed {seed}");
        for (p, &(x, z)) in m.outer.iter().zip(table::OUTER_OUTLINE.iter()) {
            assert!(dist(*p, (from_int(x), from_int(z))) <= meters(3), "seed {seed}");
        }
        // the Fold is near the top end, centered across the map
        // its five corners average 313.8 m, less 1 m of nudge
        assert!(m.fold.center.0 >= meters(312), "seed {seed}");
        assert!(
            (m.fold.center.1 - table::WIDTH / 2).abs() <= meters(4),
            "seed {seed}"
        );
        assert!(m.in_fold(m.fold.back.0, m.fold.back.1), "seed {seed}");
        assert!(m.in_fold(m.fold.center.0, m.fold.center.1), "seed {seed}");
    }
}

#[test]
fn every_map_size_is_its_own_length_and_width_with_the_fold_in_its_place() {
    // Small is 195 by 105 and Medium 244 by 131; each corner moves 2 m at most
    for (size, length, width) in [
        (MapSize::Small, 195, 105),
        (MapSize::Medium, 244, 131),
        (MapSize::Big, 325, 175),
    ] {
        for seed in 0..50 {
            let m = Map::sized(seed, size);
            let xs = || m.outer.iter().map(|p| p.0);
            let zs = || m.outer.iter().map(|p| p.1);
            let (long, wide) = (xs().max().unwrap(), zs().max().unwrap() - zs().min().unwrap());
            assert!((long - meters(length)).abs() <= meters(3), "{size:?} seed {seed}");
            assert!((wide - meters(width)).abs() <= meters(5), "{size:?} seed {seed}");
            // the Fold's five corners average 313.8 m on Big, 11.2 m from the end
            assert!(
                m.fold.center.0 >= meters(length) - meters(13),
                "{size:?} seed {seed}"
            );
            let off = m.fold.center.1 - meters(width) / 2;
            assert!(off.abs() <= meters(4), "{size:?} seed {seed}");
            // the same Fold on every size: 26 m deep and 45 m across, give
            // or take a meter of nudge at each corner
            let corners = &m.fold.outline;
            let deep = corners[2].0 - corners[0].0;
            let across = corners[4].1 - corners[0].1;
            assert!((deep - meters(26)).abs() <= meters(2), "{size:?} seed {seed}");
            assert!((across - meters(45)).abs() <= meters(2), "{size:?} seed {seed}");
        }
    }
}

#[test]
fn a_seed_and_a_map_size_give_the_same_map_every_time_and_another_size_a_different_one() {
    for size in MapSize::ALL {
        assert_eq!(Map::sized(9, size), Map::sized(9, size));
    }
    assert_ne!(
        Map::sized(9, MapSize::Small).outer,
        Map::sized(9, MapSize::Medium).outer
    );
}

#[test]
fn a_tree_in_a_gap_fails_the_route_proof() {
    // the proof itself, on a map broken by hand: stone over one wall's only
    // way through must cut the fields apart
    let mut m = Map::generate(2);
    assert_eq!(
        Flood::from(&m, m.fold.center, FLOOD_RADIUS, Mover::Sheep).sealed(),
        0
    );
    let gaps: Vec<_> = m.every_gap().copied().collect();
    for g in gaps.iter().filter(|g| g.wall == 0) {
        m.walls.push(Wall {
            a: g.a,
            b: g.b,
            half: table::WALL_HALF,
            height: table::INNER_WALL_HEIGHT,
            kind: WallKind::Outer,
        });
    }
    m.rebuild_solids();
    let flood = Flood::from(&m, m.fold.center, FLOOD_RADIUS, Mover::Sheep);
    assert!(
        flood.sealed() > 1000,
        "field 0 should be cut off: {}",
        flood.sealed()
    );
    assert_eq!(flood.reached_by_place(&m)[0], 0);
}

#[test]
fn the_proof_measures_a_known_gap() {
    // the flock takes the wider gap in each wall, so the route is as wide as
    // the smallest of those, and the Fold's gap. The grid proves that much
    // less at most 1.2 m: 0.71 m to fit a chain of points in the channel, and
    // 0.5 m for the circle moving between them. See map::proof
    for seed in 0..20 {
        let m = Map::generate(seed);
        let widest_on = |wall: u8| m.every_gap().filter(|g| g.wall == wall).map(|g| g.width).max();
        let route = (0..3).filter_map(widest_on).min().unwrap_or(0);
        let got = tool_core::map::proof::route_width(&m);
        assert!(got <= route, "seed {seed}: {got} wider than {route}");
        assert!(
            got >= route - 5 * GRID_SLACK,
            "seed {seed}: {got} far under {route}"
        );
    }
}
