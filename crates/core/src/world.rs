//! The deterministic world every browser runs (BRIEF.md D23: rollback over
//! the kit's transport; only inputs cross the wire). `World` is `Clone`: flat
//! Vecs of plain structs, no pointers, so a snapshot a tick is cheap and a
//! replay from one gives the same bits.
//!
//! The rules run around `step`, in `game.rs`. Before it they change the
//! inputs (zeroing a held player) and the state (locks, `Flock::lure`,
//! `Body::calm`). After it they read what happened. The sim leaves them these
//! doors on `Flock`: `bark`, `scatter` for a missed lunge, `stampede`, `eat`
//! (with its flinch).

use crate::body::{Body, Input, Role};
use crate::flock::{Actor, Flock};
use crate::ground::Ground;
use crate::tune;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct World {
    pub tick: u32,
    pub bodies: Vec<Body>,
    pub flock: Flock,
    /// The belled sheep a wolf first came within `tune::BELL_RADIUS` of on
    /// the last tick, if any. The rules run the ring.
    pub bell_reached: Option<u16>,
}

impl World {
    pub fn new(bodies: Vec<Body>, flock: Flock) -> World {
        assert!(bodies.len() <= tune::MAX_PLAYERS);
        World {
            tick: 0,
            bodies,
            flock,
            bell_reached: None,
        }
    }

    /// One tick. `inputs` holds one `Input` per body, in order; a body past
    /// its end stands idle.
    pub fn step<G: Ground + ?Sized>(&mut self, ground: &G, inputs: &[Input]) {
        let mut actors = [Actor::default(); tune::MAX_PLAYERS];
        let mut from = [(0, 0); tune::MAX_PLAYERS];
        let mut n = 0;
        let idle = Input::default();
        for (k, body) in self.bodies.iter_mut().enumerate() {
            if body.out {
                continue;
            }
            let input = inputs.get(k).unwrap_or(&idle);
            from[n] = (body.x, body.z);
            body.step(ground, input, self.tick);
            let dog = body.role == Role::Sheepdog;
            actors[n] = Actor {
                x: body.x,
                z: body.z,
                radius: body.role.breed().radius,
                scares: dog && !body.crouched && !body.calm,
                solid: body.grounded,
                vx: body.vx,
                vz: body.vz,
            };
            n += 1;
        }
        self.flock.step(ground, &actors[..n], self.tick);
        let moved = self.bodies.iter_mut().filter(|b| !b.out);
        for (body, (actor, from)) in moved.zip(actors[..n].iter().zip(&from)) {
            body.held_back(ground, *from, self.flock.crowding(actor));
        }
        let flock = &mut self.flock;
        self.bell_reached = self
            .bodies
            .iter()
            .filter(|b| b.role == Role::Wolf && !b.out)
            .find_map(|b| flock.reach_bell(b.x, b.z));
        self.tick += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::{CROUCH, JUMP, SPRINT};
    use crate::flock::{Kind, Sheep};
    use crate::fx::{from_int, Fx, ONE};
    use crate::ground::{FlatField, Rect};
    use crate::tune::milli;

    fn flock(sheep: &[(i32, i32)]) -> Flock {
        let wide = Rect {
            min_x: from_int(-100),
            min_z: from_int(-100),
            max_x: from_int(100),
            max_z: from_int(100),
        };
        let mut f = Flock::new(wide, None, 1);
        for &(x, z) in sheep {
            f.add(Sheep::new(Kind::Ewe, from_int(x), from_int(z)));
        }
        f
    }

    fn at(role: Role, x: i32, z: i32) -> Body {
        Body::new(role, from_int(x), from_int(z))
    }

    const WALK: Input = Input {
        strafe: 0,
        forward: 127,
        yaw: 0,
        pitch: 0,
        buttons: 0,
    };

    /// A busy world: sheep, two sheepdogs and a wolf, moving.
    fn busy() -> World {
        let bodies = vec![
            at(Role::Sheepdog, -6, 0),
            at(Role::Sheepdog, 6, 8),
            at(Role::Wolf, 0, 12),
        ];
        let sheep: Vec<(i32, i32)> = (0..20).map(|k| ((k % 5) * 2, (k / 5) * 2)).collect();
        World::new(bodies, flock(&sheep))
    }

    /// A changing script, so a replay has something to get wrong.
    fn inputs(t: u32) -> [Input; 3] {
        let yaw = (t * 97) as u16;
        let press = if t % 40 == 7 { JUMP } else { 0 };
        [
            Input {
                forward: 127,
                yaw,
                buttons: press | SPRINT,
                ..Input::default()
            },
            Input {
                strafe: 100,
                yaw: yaw.wrapping_mul(3),
                ..Input::default()
            },
            Input {
                forward: 90,
                yaw: 20000,
                buttons: CROUCH,
                ..Input::default()
            },
        ]
    }

    #[test]
    fn a_copy_of_the_world_runs_on_to_the_same_bits() {
        let mut a = busy();
        for t in 0..120 {
            a.step(&FlatField, &inputs(t));
        }
        let mut b = a.clone();
        for t in 120..240 {
            a.step(&FlatField, &inputs(t));
            b.step(&FlatField, &inputs(t));
        }
        assert_eq!(a, b);
        assert_eq!(a.tick, 240);
    }

    #[test]
    fn a_rollback_replay_lands_on_the_same_bits_and_other_inputs_do_not() {
        // what the kit will do: save a tick, go on, come back, replay
        let mut live = busy();
        for t in 0..60 {
            live.step(&FlatField, &inputs(t));
        }
        let saved = live.clone();
        for t in 60..72 {
            live.step(&FlatField, &inputs(t));
        }
        let mut replay = saved.clone();
        for t in 60..72 {
            replay.step(&FlatField, &inputs(t));
        }
        assert_eq!(replay, live);
        let mut other = saved;
        for t in 60..72 {
            let mut changed = inputs(t);
            changed[2].yaw = 0;
            other.step(&FlatField, &changed);
        }
        assert_ne!(other, live);
    }

    /// A wolf in the middle of 49 sheep a meter apart, at (3, 3).
    fn packed() -> World {
        let sheep: Vec<(i32, i32)> = (0..49).map(|k| (k % 7, k / 7)).collect();
        World::new(vec![at(Role::Wolf, 3, -3)], flock(&sheep))
    }

    /// How far from the wolf the nearest sheep is, in each quarter around it.
    fn sheep_around(w: &World) -> [i32; 4] {
        let (x, z) = (w.bodies[0].x, w.bodies[0].z);
        let mut near = [0; 4];
        for s in &w.flock.sheep {
            let (dx, dz) = (s.x - x, s.z - z);
            if crate::fx::length(dx, dz) < from_int(2) {
                near[(dx > 0) as usize + 2 * (dz > 0) as usize] += 1;
            }
        }
        near
    }

    #[test]
    fn a_wolf_walks_into_a_group_and_stands_surrounded_with_nothing_scared() {
        let mut w = packed();
        // north along +z: yaw half a turn, until it stands in the middle
        let north = Input {
            forward: 127,
            yaw: 32768,
            ..Input::default()
        };
        let mut t = 0;
        while w.bodies[0].z < from_int(3) {
            w.step(&FlatField, &[north]);
            t += 1;
            assert!(t < 180, "it waded for {t} ticks");
            assert!(w.flock.sheep.iter().all(|s| s.alarm == 0), "FL-6");
        }
        for _ in 0..120 {
            w.step(&FlatField, &[Input::default()]);
        }
        let around = sheep_around(&w);
        assert!(around.iter().all(|&n| n >= 2), "{around:?}");
        assert_eq!(crate::fx::floor(w.bodies[0].x), 3, "it walked straight");
    }

    #[test]
    fn wading_through_a_group_is_slower_than_open_ground_and_never_stops() {
        // from just clear of the first row, up to a walk by then: 43 ticks
        // at 8.4 m/s is 6 m in the open
        let mut w = packed();
        let north = Input {
            forward: 127,
            yaw: 32768,
            ..Input::default()
        };
        while w.bodies[0].z < from_int(-1) {
            w.step(&FlatField, &[north]);
        }
        let start = w.bodies[0].z;
        for _ in 0..43 {
            w.step(&FlatField, &[north]);
        }
        let waded = w.bodies[0].z - start;
        assert!(waded < from_int(5), "{waded}");
        // never under two fifths of a walk: 2.4 m at the very least
        assert!(waded > milli(2400), "{waded}");
    }

    /// How far the nearest sheep stands from this spot.
    fn nearest_sheep(w: &World, x: Fx, z: Fx) -> Fx {
        let apart = w.flock.sheep.iter().map(|s| crate::fx::length(s.x - x, s.z - z));
        apart.min().unwrap()
    }

    #[test]
    fn the_group_closes_in_behind_a_wolf_that_walked_through() {
        let mut w = packed();
        let north = Input {
            forward: 127,
            yaw: 32768,
            ..Input::default()
        };
        while w.bodies[0].z < from_int(3) {
            w.step(&FlatField, &[north]);
        }
        let (x, z) = (w.bodies[0].x, w.bodies[0].z);
        // it parted them: nothing under its middle
        let parted = nearest_sheep(&w, x, z);
        assert!(parted > milli(450), "{parted}");
        while w.bodies[0].z < from_int(8) {
            w.step(&FlatField, &[north]);
        }
        for _ in 0..300 {
            w.step(&FlatField, &[Input::default()]);
        }
        let closed = nearest_sheep(&w, x, z);
        assert!(closed < parted, "{parted} to {closed}");
        assert!(closed < milli(600), "{closed}");
    }

    #[test]
    fn a_body_that_is_out_takes_no_step_and_scares_nothing() {
        let mut w = World::new(vec![at(Role::Sheepdog, 0, 0)], flock(&[(3, 0)]));
        w.bodies[0].out = true;
        for _ in 0..30 {
            w.step(&FlatField, &[WALK]);
        }
        assert_eq!(w.bodies[0].z, 0);
        assert_eq!(w.flock.sheep[0].alarm, 0);
    }

    #[test]
    fn a_standing_sheepdog_scares_and_a_crouched_one_does_not() {
        let mut open = World::new(vec![at(Role::Sheepdog, -4, 0)], flock(&[(0, 0)]));
        open.step(&FlatField, &[Input::default()]);
        assert!(open.flock.sheep[0].alarm > 0);
        let crouch = Input {
            buttons: CROUCH,
            ..Input::default()
        };
        let mut hid = World::new(vec![at(Role::Sheepdog, -4, 0)], flock(&[(0, 0)]));
        for _ in 0..30 {
            hid.step(&FlatField, &[crouch]);
        }
        assert_eq!(hid.flock.sheep[0].alarm, 0);
    }

    #[test]
    fn a_wolf_scares_nothing_and_a_calm_dog_scares_nothing() {
        let mut w = World::new(vec![at(Role::Wolf, -2, 0)], flock(&[(0, 0)]));
        for _ in 0..30 {
            w.step(&FlatField, &[Input::default()]);
        }
        assert_eq!(w.flock.sheep[0].alarm, 0);
        let mut w = World::new(vec![at(Role::Sheepdog, -2, 0)], flock(&[(0, 0)]));
        w.bodies[0].calm = true;
        for _ in 0..30 {
            w.step(&FlatField, &[Input::default()]);
        }
        assert_eq!(w.flock.sheep[0].alarm, 0);
    }

    #[test]
    fn a_body_on_the_ground_pushes_a_sheep_and_a_jumping_one_passes_over() {
        // one tick of a body half a meter from a sheep, well inside touching:
        // on the ground it shoves, and a jump leaves the ground on that tick
        let beside = |input: Input| {
            let mut w = World::new(vec![at(Role::Wolf, 0, 0)], flock(&[(0, 0)]));
            w.flock.sheep[0].x = ONE / 2;
            w.step(&FlatField, &[input]);
            w.flock.sheep[0].x
        };
        assert!(beside(Input::default()) > ONE / 2);
        assert_eq!(
            beside(Input {
                buttons: JUMP,
                ..Input::default()
            }),
            ONE / 2
        );
    }

    fn belled_world(wolf_x: i32) -> World {
        let mut w = World::new(vec![at(Role::Wolf, wolf_x, 0)], flock(&[(0, 0)]));
        w.flock.set_bell(0, true);
        w
    }

    #[test]
    fn the_world_reports_a_wolfs_first_arrival_at_a_bell_and_only_that() {
        // the bell's reach is 20 m
        let mut far = belled_world(21);
        far.step(&FlatField, &[Input::default()]);
        assert_eq!(far.bell_reached, None);
        let mut near = belled_world(19);
        near.step(&FlatField, &[Input::default()]);
        assert_eq!(near.bell_reached, Some(0));
        near.step(&FlatField, &[Input::default()]);
        assert_eq!(near.bell_reached, None, "only the first tick");
    }

    #[test]
    fn a_sheepdog_or_a_wolf_that_is_out_does_not_reach_a_bell() {
        let mut w = World::new(vec![at(Role::Sheepdog, 3, 0)], flock(&[(0, 0)]));
        w.flock.set_bell(0, true);
        w.step(&FlatField, &[Input::default()]);
        assert_eq!(w.bell_reached, None);
        let mut w = belled_world(3);
        w.bodies[0].out = true;
        w.step(&FlatField, &[Input::default()]);
        assert_eq!(w.bell_reached, None);
    }
}
