//! Every balance number the sim uses, in one table (BRIEF.md: the person's
//! numbers were spitballed, so each one must be able to move). Starting
//! values come from `notes/contract.md`. Meters and seconds; `Fx` is Q16.16.

use crate::fx::Fx;

/// Ticks a second. The one number lives in `ticks.rs`, the clock that runs
/// them.
pub const TICKS_PER_SECOND: i32 = crate::ticks::TICKS_PER_SECOND as i32;

/// `n` thousandths as a fixed-point value, for the tables below.
pub const fn milli(n: i32) -> Fx {
    ((n as i64 * 65536) / 1000) as Fx
}

/// Whole seconds as ticks.
pub const fn seconds(n: i32) -> i32 {
    n * TICKS_PER_SECOND
}

/// A rate per second as a rate per tick.
pub const fn per_tick(v: Fx) -> Fx {
    v / TICKS_PER_SECOND
}

/// What a team's body can do and how big it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Breed {
    pub walk: Fx,
    pub sprint: Fx,
    pub crouch: Fx,
    pub length: Fx,
    pub shoulder: Fx,
    pub shoulder_crouched: Fx,
    /// The circle that walls, trees and sheep meet.
    pub radius: Fx,
}

pub const SHEEPDOG: Breed = Breed {
    walk: milli(9000),
    sprint: milli(16000),
    crouch: milli(4400),
    length: milli(950),
    shoulder: milli(560),
    shoulder_crouched: milli(380),
    radius: milli(360),
};

pub const WOLF: Breed = Breed {
    walk: milli(8400),
    sprint: milli(14800),
    crouch: milli(4400),
    length: milli(1250),
    shoulder: milli(800),
    shoulder_crouched: milli(600),
    radius: milli(420),
};

/// The most players in a match (BRIEF.md).
pub const MAX_PLAYERS: usize = 8;

// Bodies

pub const GRAVITY: Fx = milli(20_000);
/// Lifts a body about 1.4 m: speed squared over twice gravity. Inner walls
/// stand 0.9 m above the ground at the wall, and going uphill that ground
/// lies 0.25 m above a running takeoff. A sprint clears such a wall leaving
/// the ground from about 1 to 3.5 m out from its line, and a walk from 1 to
/// 4 m: a body that meets the stone rises beside it, and `AIR_ACCEL` carries
/// it over.
pub const JUMP_SPEED: Fx = milli(7600);
/// The fastest a wall moves a body. Past a Zoomies sprint, so a wall stops any body
/// outright, and it eases a body whose feet drop below a wall's top while
/// over it, which the map would otherwise pop off by up to a meter a tick.
pub const WALL_EASE: Fx = milli(30_000);
/// How fast a body reaches its speed on the ground and in the air: a walk in
/// about a tenth of a second, and a third of that rate in the air.
pub const GROUND_ACCEL: Fx = milli(80_000);
pub const AIR_ACCEL: Fx = milli(24_000);
/// A floor that falls away by more than this in one tick is an edge, not a
/// slope: the body leaves the ground.
pub const STEP_DOWN: Fx = milli(300);

/// A full bar lasts this long at a sprint, and refills in this long.
pub const SPRINT_TICKS: i32 = seconds(6);
pub const REFILL_TICKS: i32 = seconds(5);
/// In stamina units, so the bar is exact in whole ticks: a tick at a sprint
/// costs `REFILL_TICKS` and a tick of rest gives back `SPRINT_TICKS`.
pub const STAMINA_FULL: i32 = SPRINT_TICKS * REFILL_TICKS;
/// A body that ran the bar dry cannot sprint again until it refills this far
/// (a fifth), so holding the key does not stutter.
pub const WINDED_UNTIL: i32 = STAMINA_FULL / 5;

// Sheep sizes. A sheep's `size` scales the ewe of the contract's table.

pub const SHEEP_RADIUS: Fx = milli(450);
pub const EWE_SPREAD: Fx = milli(120);
/// 1.15 m over the ewe's 1.05 m.
pub const RAM_SIZE: Fx = milli(1095);
/// 0.60 m over the ewe's 1.05 m, and a little either way.
pub const LAMB_SIZE: Fx = milli(571);
pub const LAMB_SPREAD: Fx = milli(50);
/// One sheep in this many is a lamb, and one adult in this many a ram.
pub const LAMB_ONE_IN: u32 = 6;
/// A lamb starts by the nearest ewe this close to its spot.
pub const LAMB_FINDS_EWE: Fx = milli(30_000);
pub const RAM_ONE_IN: u32 = 16;
/// Fleece colors, weighted: the models own the palette; this says how many
/// sheep wear each index.
pub const FLEECE_WEIGHTS: [u32; 6] = [52, 14, 12, 10, 7, 5];

// Sheep speeds, from the contract.

pub const GRAZE_DRIFT: Fx = milli(800);
pub const FLOCK_TROT: Fx = milli(4800);
pub const FLEE: Fx = milli(10000);
/// How a sheep leaves at the rim of a sheepdog's flight zone.
pub const WALK_OFF: Fx = milli(2000);

// Fear

/// Steering runs for each sheep on one tick in this many; movement runs on
/// every tick.
pub const STEER_EVERY: u32 = 3;
/// A sheep starts to move off when a sheepdog is this near, and is at full
/// flee when it is as near as the core.
pub const FLIGHT_RADIUS: Fx = milli(9000);
pub const FLIGHT_CORE: Fx = milli(3000);
/// The alarm of a sheep at the very rim of a flight zone: it has seen the
/// sheepdog and is on its toes, so it bunches up even as it barely moves.
pub const ALERT: Fx = milli(600);
/// A sheep picks up this share of the alarm around it, so the alarm fades as
/// it spreads through a flock.
pub const CONTAGION: Fx = milli(800);
/// A frightened sheep that wants to go at this speed or more, and has hardly
/// moved for this long, makes a run for it.
pub const STUCK_WANT: Fx = milli(500);
pub const STUCK_TICKS: u8 = 18;
/// It walks each line it might take, this many ways round, in steps shorter
/// than a wall is thick, so a look never lands past a wall.
pub const BREAK_LINES: i32 = 8;
pub const BREAK_LOOK: Fx = milli(800);
pub const BREAK_STEPS: usize = 3;
/// What a line away from the fear or toward the flock is worth, in meters of
/// open run. Small: the open line matters most, even past the dog.
pub const BREAK_LEAN: Fx = milli(500);
/// How long its run lasts.
pub const BREAK_OUT_TICKS: u16 = 42;
/// An alarmed sheep calms from full to nothing in this many ticks.
pub const ALARM_FADE_TICKS: i32 = seconds(3);

// The flock

/// How far a calm and an alarmed sheep count neighbors for their local
/// center. A calm one sees its own group, so groups do not gather by
/// themselves; a frightened one runs for any flock in sight.
pub const SIGHT_CALM: Fx = milli(7000);
pub const SIGHT_ALARMED: Fx = milli(15_000);
/// A calm sheep farther than this from its local center walks back.
pub const COMFORT: Fx = milli(1500);
/// Speed toward the local center per meter away, up to the most, for a sheep
/// in full alarm. It is added to the mean velocity around the sheep, so it
/// closes the flock up without turning its front around.
pub const COHESION_GAIN: Fx = milli(2500);
pub const COHESION_MAX: Fx = milli(4800);
/// The share of its neighbors' velocity an alarmed sheep takes on.
pub const ALIGNMENT: Fx = milli(900);
/// How a sheep's velocity follows its wish: a share of the difference a tick
/// (one over this).
pub const INERTIA_TICKS: i32 = 8;
/// Speed noise, as a share of a moving sheep's speed.
pub const WANDER: Fx = milli(150);

/// Center spacing as a share of two sheep's radii added: calm and alarmed.
/// Under one means they overlap, which fluffy sheep may. At rest they stand
/// shoulder to shoulder (the person: "stick closer together").
pub const SPACING_CALM: Fx = milli(1150);
pub const SPACING_TIGHT: Fx = milli(880);
/// How hard a sheep slides out of a neighbor, at a full overlap, and at most.
pub const SHOVE: Fx = milli(4000);
pub const SHOVE_MAX: Fx = milli(2400);

// Edges

/// A sheep feels a wall, tree or cairn this far from its own side, and turns
/// fully along it this far from it, so inertia does not carry it onto the
/// stone.
pub const EDGE_SENSE: Fx = milli(1500);
pub const EDGE_CLEAR: Fx = milli(600);
/// How strongly the flock settles which way along an edge a sheep goes: this
/// many m/s of the choice for each meter the flock lies along it. Beside a
/// gap the flock is what says which way is out.
pub const EDGE_TO_FLOCK: i32 = 2;
/// How fast a frightened sheep eases off an edge it is touching. A calm one
/// drifts off at `GRAZE_DRIFT`.
pub const EDGE_PUSH: Fx = milli(1600);

// Grazing: each sheep's own slow round of grazing, walking and turning

pub const GRAZE_MIN_TICKS: i32 = seconds(3);
pub const GRAZE_MAX_TICKS: i32 = seconds(8);
pub const AMBLE_MIN_TICKS: i32 = seconds(2);
pub const AMBLE_MAX_TICKS: i32 = seconds(5);
/// A sheep's own pace, set once from the seed: its grazes take this percent
/// of the usual time and its walks the inverse.
pub const TEMPO_MIN: i32 = 70;
pub const TEMPO_MAX: i32 = 140;
/// A sheep with nobody in sight walks this percent of the usual time.
pub const LONE_WALK_PERCENT: i32 = 160;
/// The most a walk bends, in radians a steering tick: over a walk of four
/// seconds, about 60 degrees.
pub const AMBLE_BEND: Fx = milli(14);
/// How fast a stopped sheep turns, in radians a steering tick, and the time
/// that takes over a half turn.
pub const TURN_BEND: Fx = milli(78);
pub const HALF_TURN_TICKS: i32 = seconds(2);
/// How hard a sheep at the edge of its group leans its next walk back in. At
/// 4 a sheep at `COMFORT` heads within 15 degrees of its group's middle.
pub const GROUP_LEAN: i32 = 4;
/// How far ahead a calm sheep looks for the line between two fields.
pub const FIELD_LOOK: Fx = milli(300);
/// Above this speed a sheep faces the way it is going.
pub const FACE_SPEED: Fx = milli(200);

// Lambs

/// A lamb keeps this near its ewe.
pub const LAMB_LEASH: Fx = milli(1200);
pub const LAMB_GAIN: Fx = milli(3000);

// Players among the sheep

/// A player shifts a sheep this fast at most, however fast it walks.
pub const PUSH: Fx = milli(4500);
/// How near a body and a sheep come, as a share of their radii added: fleece
/// gives, so a body stands in among sheep and doesn't hold them off at arm's
/// length (the person: "collide with the sheep more and pack in with them").
pub const BODY_TOUCH: Fx = milli(800);
/// How a sheep ahead of a body goes aside: this much across for each meter
/// straight away, so a body parts sheep and doesn't plow them along.
pub const PART: Fx = milli(1200);
/// Each sheep a body is pressing into takes this share of its step, up to the
/// most, so it wades through a group and can never be stopped by one.
pub const CROWD_DRAG: Fx = milli(250);
pub const CROWD_MOST: Fx = milli(600);

// The fold

/// A folded sheep walks this fast.
pub const FOLD_AMBLE: Fx = milli(1800);
/// Folded sheep fill a half circle at the back: its radius in meters is this
/// many times the square root of how many are in.
pub const FOLD_PACKING: Fx = milli(1000);

// The carrot

pub const LURE_RADIUS: Fx = milli(16_000);
/// Sheep crowd the carrot this near.
pub const LURE_CLOSE: Fx = milli(1500);

/// A one-off fright from a point: how far it reaches, how fast sheep run at
/// the point and at the edge of its reach, and how long they run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fright {
    pub radius: Fx,
    pub near_speed: Fx,
    pub far_speed: Fx,
    pub ticks: u16,
}

/// A bark: stronger and wider than a sheepdog's plain presence.
pub const BARK: Fright = Fright {
    radius: milli(16_000),
    near_speed: FLEE,
    far_speed: FLOCK_TROT,
    ticks: 36,
};
/// A missed lunge.
pub const SCATTER: Fright = Fright {
    radius: milli(8000),
    near_speed: FLEE,
    far_speed: FLOCK_TROT,
    ticks: 60,
};
/// The Stampede power-up: stronger and wider than a scatter, and it lasts.
pub const STAMPEDE: Fright = Fright {
    radius: milli(15_000),
    near_speed: milli(11_000),
    far_speed: milli(9000),
    ticks: 210,
};

/// What a wolf's meal does to the sheep around it: a hop, and nothing lasting.
pub const FLINCH_RADIUS: Fx = milli(5000);
pub const FLINCH_KICK: Fx = milli(1500);
pub const HOP_TICKS: u8 = 18;
pub const HOP_HEIGHT: Fx = milli(250);

/// The Bell power-up: the world reports a wolf's first time this near a
/// belled sheep, and the rules ring it.
pub const BELL_RADIUS: Fx = milli(20_000);
