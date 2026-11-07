//! Scenarios for the thing the whole game rests on: a sheepdog moves a flock
//! the way a real one does. Each runs the real `World` from real `Input`.
//! Expected numbers come from the speeds and sizes in `tune.rs`; the working
//! is beside each.

mod common;

use common::*;
use tool_core::flock::{Flock, Fold, Kind, Sheep};
use tool_core::fx::from_int;
use tool_core::ground::{FlatField, WalledField};
use tool_core::tune;
use tool_core::world::World;

const CALM_30: f64 = 1.8;

/// A flock on open land with no fold.
fn open_flock(count: usize, spacing: f64) -> Flock {
    let bounds = rect(-200.0, -200.0, 200.0, 200.0);
    let mut flock = Flock::new(bounds, None, 3);
    let side = (count as f64).sqrt().ceil() as usize;
    for k in 0..count {
        let x = (k % side) as f64 * spacing;
        let z = (k / side) as f64 * spacing;
        flock.add(Sheep::new(Kind::Ewe, m(x), m(z)));
    }
    flock
}

/// A dog working the flock toward a point far to the east on open land, with
/// `each` seeing the free sheep once a tick. A dog that simply walked on would
/// pass through sheep that trot at half its pace, so this one keeps its post
/// behind them, as a player does.
fn drive_east(world: &mut World, seconds: u32, mut each: impl FnMut(&[(f64, f64)])) {
    let z = center(&free_sheep(&world.flock)).1;
    let mut dog = Shepherd::toward(150.0, z);
    for _ in 0..seconds * SECOND {
        let input = dog.input(&FlatField, &world.bodies[0], &world.flock);
        world.step(&FlatField, &[input]);
        each(&free_sheep(&world.flock));
    }
}

#[test]
fn a_dog_on_the_far_side_of_a_flock_drives_it_straight_away() {
    // The dog starts 10 m behind 30 sheep. At a trot of 2.4 m/s, 20 s is 48 m
    // less the seconds the dog takes to close and the flock to tighten, and
    // the flock must go down the line rather than off to a side.
    let flock = open_flock(30, CALM_30);
    let mut world = world_with_dog(-8.0, 4.0, flock);
    let start = center(&free_sheep(&world.flock));
    drive_east(&mut world, 20, |_| {});
    let end = center(&free_sheep(&world.flock));
    assert!(end.0 - start.0 > 30.0, "{start:?} {end:?}");
    assert!((end.1 - start.1).abs() < 6.0, "{start:?} {end:?}");
}

#[test]
fn the_leading_edge_of_a_driven_flock_stays_with_the_flock() {
    // Tight at 0.8 m a sheep, 30 sheep fill a disk of about 2.5 m. If the
    // front bolted it would pull away at 2.6 m/s, flee speed over trot: 26 m
    // in the 10 s measured here. It must stay with the flock.
    let flock = open_flock(30, CALM_30);
    let mut world = world_with_dog(-8.0, 4.0, flock);
    let mut worst_lead: f64 = 0.0;
    let mut worst_reach: f64 = 0.0;
    let mut tick = 0;
    drive_east(&mut world, 20, |sheep| {
        tick += 1;
        // the first seconds are the loose start closing up
        if tick > 10 * SECOND {
            let c = center(sheep);
            let front = sheep.iter().map(|s| s.0).fold(f64::MIN, f64::max);
            worst_lead = worst_lead.max(front - c.0);
            worst_reach = worst_reach.max(reach(sheep));
        }
    });
    assert!(worst_lead < 6.0, "front led by {worst_lead}");
    assert!(worst_reach < 8.0, "farthest sheep {worst_reach}");
}

#[test]
fn a_big_flock_is_driven_as_one() {
    let flock = open_flock(100, CALM_30);
    let mut world = world_with_dog(-8.0, 9.0, flock);
    let mut worst: f64 = 0.0;
    let mut tick = 0;
    drive_east(&mut world, 30, |sheep| {
        tick += 1;
        if tick > 15 * SECOND {
            worst = worst.max(reach(sheep));
        }
    });
    let c = center(&free_sheep(&world.flock));
    // 100 tight sheep fill a disk of about 4.5 m
    assert!(worst < 12.0, "farthest sheep {worst}");
    assert!(c.0 > 35.0, "{c:?}");
}

#[test]
fn a_scripted_dog_drives_thirty_sheep_through_a_gap_and_into_the_fold() {
    // The gap is 6 m and the fold's gate the same. At the contract's speeds
    // the 100 m from the start to the fold is 40 s at a trot; a third again
    // leaves room for the dog to sweep behind the flock.
    for seed in [1, 2, 3, 4, 5] {
        let p = pasture();
        let flock = loose_flock(&p, 30, -25.0, 8.0, CALM_30, seed);
        let mut world = world_with_dog(-50.0, 0.0, flock);
        let mut dog = shepherd(&p);
        let mut first_through = None;
        let mut all_through = None;
        let mut east = [false; 30];
        let mut went_back = 0;
        for t in 0..60 * SECOND {
            let input = dog.input(&p.ground, &world.bodies[0], &world.flock);
            world.step(&p.ground, &[input]);
            let mut through = 0;
            for (k, s) in world.flock.sheep.iter().enumerate() {
                let is_east = f(s.x) > p.wall_x;
                went_back += (east[k] && !is_east) as u32;
                east[k] = is_east;
                through += is_east as u32;
            }
            if through > 0 && first_through.is_none() {
                first_through = Some(t);
            }
            if through == 30 && all_through.is_none() {
                all_through = Some(t);
            }
        }
        assert_eq!(world.flock.folded, 30, "seed {seed}");
        // no jam: 30 sheep clear a 6 m gap in a few seconds once they start
        let (from, to) = (first_through.expect("through"), all_through.expect("all through"));
        assert!(to - from < 8 * SECOND, "seed {seed}: {} s", (to - from) / SECOND);
        // no bad leak: a sheep may step back in the gap, not the flock
        assert!(went_back <= 2, "seed {seed}: {went_back}");
    }
}

#[test]
fn a_hundred_sheep_flow_through_the_same_gaps() {
    let p = pasture();
    let flock = loose_flock(&p, 100, -25.0, 8.0, CALM_30, 7);
    let mut world = world_with_dog(-50.0, 0.0, flock);
    let mut dog = shepherd(&p);
    for _ in 0..200 * SECOND {
        let input = dog.input(&p.ground, &world.bodies[0], &world.flock);
        world.step(&p.ground, &[input]);
    }
    assert_eq!(world.flock.folded, 100);
}

#[test]
fn a_flock_left_alone_stays_grouped_but_loosens_to_graze() {
    // Sheep on top of each other ease out to calm spacing, about 1 m, so 30
    // of them fill a disk of about 3 m, and two minutes of ambling must not
    // dissolve it.
    let mut flock = open_flock(30, 0.5);
    let start = reach(&free_sheep(&flock));
    let mut world = {
        // a dog nobody can see, far away
        flock.sheep.iter_mut().for_each(|s| s.alarm = from_int(1));
        world_with_dog(-100.0, -100.0, flock)
    };
    for _ in 0..120 * SECOND {
        world.step(&FlatField, &[]);
    }
    let sheep = free_sheep(&world.flock);
    assert!(reach(&sheep) > start + 2.0, "{start} {}", reach(&sheep));
    assert!(reach(&sheep) < 6.0, "{}", reach(&sheep));
}

#[test]
fn groups_far_apart_do_not_gather_by_themselves() {
    // Calm sheep see 7 m. Two groups in a line, 7.8 m between their nearest
    // sheep, are out of each other's sight and must stay apart for two
    // minutes, or the dog has no gathering to do. Each draws in on itself, so
    // it is the gap between them that counts, not their centers.
    let bounds = rect(-100.0, -100.0, 100.0, 100.0);
    let mut flock = Flock::new(bounds, None, 9);
    for k in 0..5 {
        flock.add(Sheep::new(Kind::Ewe, m(k as f64 * 1.8), 0));
        flock.add(Sheep::new(Kind::Ewe, m(15.0 + k as f64 * 1.8), 0));
    }
    let mut world = world_with_dog(-90.0, -90.0, flock);
    for _ in 0..120 * SECOND {
        world.step(&FlatField, &[]);
    }
    let sheep = free_sheep(&world.flock);
    let west: Vec<_> = sheep.iter().copied().step_by(2).collect();
    let east: Vec<_> = sheep.iter().copied().skip(1).step_by(2).collect();
    let gap = west
        .iter()
        .flat_map(|&a| east.iter().map(move |&b| dist(a, b)))
        .fold(f64::MAX, f64::min);
    assert!(gap > 5.0, "{gap}: {:?} {:?}", center(&west), center(&east));
}

#[test]
fn scared_sheep_run_for_a_flock_in_sight() {
    // A lone sheep 10 m from a flock is past its calm sight of 7 m. Alarmed
    // at the 0.6 a dog's flight zone gives at the least, it sees 11.8 m, so
    // it heads for the others rather than straight away from the dog.
    let bounds = rect(-100.0, -100.0, 100.0, 100.0);
    let mut flock = Flock::new(bounds, None, 11);
    for k in 0..9 {
        flock.add(Sheep::new(
            Kind::Ewe,
            m((k % 3) as f64 * 1.2),
            m((k / 3) as f64 * 1.2),
        ));
    }
    let lone = flock.add(Sheep::new(Kind::Ewe, m(1.2), m(11.2)));
    let mut world = world_with_dog(7.2, 11.2, flock);
    for _ in 0..4 * SECOND {
        world.step(&FlatField, &[]);
    }
    let s = &world.flock.sheep[lone];
    let others = free_sheep(&world.flock);
    let c = center(&others[..9]);
    assert!(dist((f(s.x), f(s.z)), c) < 7.0, "{:?} {c:?}", (f(s.x), f(s.z)));
}

/// How many separate groups these sheep make, joined by sight.
fn groups(sheep: &[(f64, f64)], sight: f64) -> usize {
    let mut seen = vec![false; sheep.len()];
    let mut groups = 0;
    for start in 0..sheep.len() {
        if seen[start] {
            continue;
        }
        groups += 1;
        let mut open = vec![start];
        seen[start] = true;
        while let Some(k) = open.pop() {
            for j in 0..sheep.len() {
                if !seen[j] && dist(sheep[k], sheep[j]) < sight {
                    seen[j] = true;
                    open.push(j);
                }
            }
        }
    }
    groups
}

#[test]
fn a_stampede_runs_then_the_flock_calms_and_regroups() {
    let flock = open_flock(30, 1.2);
    let mut world = world_with_dog(-90.0, -90.0, flock);
    let c = center(&free_sheep(&world.flock));
    world.flock.stampede(m(c.0 - 6.0), m(c.1));
    // three seconds on, at over 4.5 m/s it is a long way off and still running
    for _ in 0..3 * SECOND {
        world.step(&FlatField, &[]);
    }
    let away = center(&free_sheep(&world.flock));
    assert!(dist(away, c) > 10.0, "{away:?}");
    assert!(world.flock.sheep.iter().all(|s| s.alarm > 0));
    // half a minute later it is calm and back together, give or take the
    // one straggler a bolt like that leaves for the dog to fetch
    for _ in 0..30 * SECOND {
        world.step(&FlatField, &[]);
    }
    assert!(world.flock.sheep.iter().all(|s| s.alarm == 0));
    let sheep = free_sheep(&world.flock);
    assert!(groups(&sheep, f(tune::SIGHT_CALM)) <= 2, "{sheep:?}");
}

#[test]
fn a_wolfs_meal_makes_neighbors_flinch_and_the_flock_stays_calm() {
    let flock = open_flock(9, 1.5);
    let mut world = world_with_dog(-90.0, -90.0, flock);
    let before = free_sheep(&world.flock);
    world.flock.eat(4);
    for _ in 0..2 * SECOND {
        world.step(&FlatField, &[]);
    }
    assert!(world.flock.sheep.iter().all(|s| s.alarm == 0 && s.bolt == 0));
    let after = free_sheep(&world.flock);
    assert_eq!(after.len(), 8);
    assert!(dist(center(&before), center(&after)) < 1.0);
}

/// How far each sheep walks in `seconds` with nothing to bother it.
fn walked(world: &mut World, seconds: u32) -> Vec<f64> {
    let mut path = vec![0.0; world.flock.sheep.len()];
    for _ in 0..seconds * SECOND {
        let before: Vec<(f64, f64)> = world.flock.sheep.iter().map(|s| (f(s.x), f(s.z))).collect();
        world.step(&FlatField, &[]);
        for (k, s) in world.flock.sheep.iter().enumerate() {
            path[k] += dist(before[k], (f(s.x), f(s.z)));
        }
    }
    path
}

#[test]
fn a_calm_sheep_covers_some_ground_in_a_minute_and_stays_near_its_group() {
    // A round of about ten seconds with some 3.5 s of it walking at 0.8 m/s
    // is about 16 m a minute, less the sheep that stop early to turn back.
    let mut world = world_with_dog(-90.0, -90.0, open_flock(16, CALM_30));
    let path = walked(&mut world, 60);
    let fewest = path.iter().copied().fold(f64::MAX, f64::min);
    let mean = path.iter().sum::<f64>() / path.len() as f64;
    assert!(fewest > 2.0, "{path:?}");
    assert!(mean > 4.0 && mean < 14.0, "{path:?}");
    let sheep = free_sheep(&world.flock);
    assert!(reach(&sheep) < f(tune::SIGHT_CALM), "{}", reach(&sheep));
}

#[test]
fn a_calm_flock_moves_slowly_and_does_not_jitter() {
    // Twelve sheep bunched up, then left. Sampled a fifth of a second apart,
    // a sheep that reverses its way of going is a sheep twitching; a turn at
    // the end of a walk comes about once in a round, so a few a minute.
    let mut world = world_with_dog(-90.0, -90.0, open_flock(12, 0.9));
    for _ in 0..20 * SECOND {
        world.step(&FlatField, &[]);
    }
    let mut last: Vec<(f64, f64)> = vec![(0.0, 0.0); 12];
    let mut reversed = vec![0; 12];
    for t in 0..60 * SECOND {
        world.step(&FlatField, &[]);
        if t % (SECOND / 5) != 0 {
            continue;
        }
        for (k, s) in world.flock.sheep.iter().enumerate() {
            let now = (f(s.vx), f(s.vz));
            assert!(len(s.vx, s.vz) < 0.82, "{now:?}");
            let moving = len(s.vx, s.vz) > 0.05 && (last[k].0.abs() + last[k].1.abs()) > 0.05;
            reversed[k] += (moving && now.0 * last[k].0 + now.1 * last[k].1 < 0.0) as i32;
            last[k] = now;
        }
    }
    assert!(reversed.iter().all(|&r| r <= 10), "{reversed:?}");
}

#[test]
fn a_herded_group_drifts_slowly_and_stays_a_group() {
    // Left alone for three minutes by the dog that bunched it.
    let mut world = world_with_dog(-90.0, -90.0, open_flock(30, 1.0));
    let start = center(&free_sheep(&world.flock));
    walked(&mut world, 180);
    let sheep = free_sheep(&world.flock);
    assert!(
        dist(center(&sheep), start) < 5.0,
        "{:?} {start:?}",
        center(&sheep)
    );
    assert_eq!(groups(&sheep, f(tune::SIGHT_CALM)), 1);
    // and as tight as it rests: 30 sheep a meter apart fill a disk of 3 m
    assert!(reach(&sheep) < 5.0, "{}", reach(&sheep));
}

#[test]
fn a_lone_sheep_drifts_further_than_one_in_a_group() {
    // Lone: each walk is 1.6 times the usual and none is cut short.
    let mut lone = Flock::new(rect(-200.0, -200.0, 200.0, 200.0), None, 21);
    for k in 0..16 {
        lone.add(Sheep::new(Kind::Ewe, m(k as f64 * 20.0 - 150.0), 0));
    }
    let mut alone = world_with_dog(-190.0, -190.0, lone);
    let mut group = world_with_dog(-90.0, -90.0, open_flock(16, CALM_30));
    let mean = |path: Vec<f64>| path.iter().sum::<f64>() / path.len() as f64;
    let (far, near) = (mean(walked(&mut alone, 120)), mean(walked(&mut group, 120)));
    assert!(far > near * 1.2, "{far} {near}");
}

#[test]
fn a_settled_flock_by_a_gap_does_not_leak_back() {
    // Just through the wall and left there for three minutes.
    let p = pasture();
    let mut flock = Flock::new(p.bounds, Some(p.fold), 5);
    for k in 0..30 {
        let (x, z) = (1.2 + (k % 6) as f64 * 1.5, (k / 6) as f64 * 1.5 - 3.0);
        flock.add(Sheep::new(Kind::Ewe, m(x), m(z)));
    }
    let mut world = world_with_dog(-50.0, 0.0, flock);
    for _ in 0..180 * SECOND {
        world.step(&p.ground, &[]);
        let back = world.flock.sheep.iter().filter(|s| f(s.x) < p.wall_x).count();
        assert_eq!(back, 0);
    }
}

#[test]
fn a_fold_stands_at_any_side_of_the_field() {
    // a fold in the south-west corner with its gap facing east, so nothing
    // in the flock depends on the fold lying to the east
    let ground = WalledField {
        fold: Some(rect(-40.0, -40.0, -20.0, -20.0)),
        ..WalledField::default()
    };
    let fold = Fold {
        back_x: m(-40.0),
        back_z: m(-30.0),
    };
    let mut flock = Flock::new(rect(-60.0, -60.0, 60.0, 60.0), Some(fold), 1);
    flock.add(Sheep::new(Kind::Ewe, m(-16.0), m(-30.0)));
    let mut world = world_with_dog(0.0, 0.0, flock);
    world.flock.bark(m(-13.0), m(-30.0));
    for _ in 0..30 * SECOND {
        world.step(&ground, &[]);
    }
    assert_eq!(world.flock.folded, 1);
    assert!(f(world.flock.sheep[0].x) < -30.0, "{}", f(world.flock.sheep[0].x));
}
