//! A player's body: walking, sprinting, crouching, jumping, falling, and the
//! stamina bar. One tick of one body from one `Input`.
//!
//! Facing, for everyone who turns a yaw into a direction: yaw 0 looks along
//! -z and a quarter turn looks along +x, so a growing yaw turns right seen
//! from above. Forward is (sin yaw, -cos yaw), right is (cos yaw, sin yaw).
//! That matches a right-handed world with y up.

use crate::flock::{approx_len, at_most};
use crate::fx::{self, Fx};
use crate::ground::Ground;
use crate::map::Mover;
pub use crate::rules::Role;
use crate::tune::{self, per_tick, Breed};

/// Held to run. Spends stamina.
pub const SPRINT: u16 = 1 << 0;
/// Held to crouch: slower, lower, and sheep ignore a crouched sheepdog.
pub const CROUCH: u16 = 1 << 1;
/// Jump, on the press.
pub const JUMP: u16 = 1 << 2;
/// E. `game.rs` says what it means: a mound or a tunnel mouth under the
/// crosshair first, otherwise a Sheepdog's bark.
pub const ACT: u16 = 1 << 3;
/// Left click: the rules' lunge or eat.
pub const PRIMARY: u16 = 1 << 4;
/// Right click: the rules' power-up.
pub const SECONDARY: u16 = 1 << 5;
/// F: the rules' discard.
pub const DISCARD: u16 = 1 << 6;

/// One player's wishes for one tick. Eight bytes, plain, and comparable, so
/// the rollback kit can send it, predict it and spot a misprediction.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Input {
    /// -127 to 127, right positive.
    pub strafe: i8,
    /// -127 to 127, forward positive.
    pub forward: i8,
    /// A whole turn is 65536 and wraps by itself.
    pub yaw: u16,
    /// Up positive, in the same angle units.
    pub pitch: i16,
    pub buttons: u16,
}

impl Input {
    /// As one number, for the wire: an idle input is 0.
    pub fn pack(self) -> u64 {
        (self.strafe as u8 as u64)
            | (self.forward as u8 as u64) << 8
            | (self.yaw as u64) << 16
            | (self.pitch as u16 as u64) << 32
            | (self.buttons as u64) << 48
    }

    pub fn unpack(keys: u64) -> Input {
        Input {
            strafe: keys as u8 as i8,
            forward: (keys >> 8) as u8 as i8,
            yaw: (keys >> 16) as u16,
            pitch: (keys >> 32) as u16 as i16,
            buttons: (keys >> 48) as u16,
        }
    }
}

impl Role {
    pub fn breed(self) -> &'static Breed {
        match self {
            Role::Sheepdog => &tune::SHEEPDOG,
            Role::Wolf => &tune::WOLF,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Body {
    pub role: Role,
    pub x: Fx,
    pub y: Fx,
    pub z: Fx,
    pub vx: Fx,
    pub vy: Fx,
    pub vz: Fx,
    pub yaw: u16,
    pub pitch: i16,
    /// `tune::STAMINA_FULL` when full.
    pub stamina: i32,
    pub grounded: bool,
    pub crouched: bool,
    pub sprinting: bool,
    /// Ran the bar dry and has not yet refilled to `tune::WINDED_UNTIL`.
    pub winded: bool,
    /// Off the field: killed, or down a tunnel. Scares and pushes nothing,
    /// and takes no step. Set by the rules.
    pub out: bool,
    /// A sheepdog that does not scare sheep although it stands in the open,
    /// such as one holding out a carrot. Set by the rules.
    pub calm: bool,
    /// The body stands still before this tick: eating, digging, stunned by
    /// the bone. It can still look around. Set by the rules.
    pub locked_until: u32,
    /// Multiplies sprint speed. Set by the rules for zoomies.
    pub sprint_scale: Fx,
    /// Last tick's buttons, so a press is seen once.
    pub held: u16,
}

impl Body {
    pub fn new(role: Role, x: Fx, z: Fx) -> Body {
        Body {
            role,
            x,
            y: 0,
            z,
            vx: 0,
            vy: 0,
            vz: 0,
            yaw: 0,
            pitch: 0,
            stamina: tune::STAMINA_FULL,
            grounded: true,
            crouched: false,
            sprinting: false,
            winded: false,
            out: false,
            calm: false,
            locked_until: 0,
            sprint_scale: fx::ONE,
            held: 0,
        }
    }

    /// Stand on the land here, for spawning and for rules that move a body.
    pub fn settle<G: Ground + ?Sized>(&mut self, ground: &G) {
        self.y = ground.height(self.x, self.z);
        self.vy = 0;
        self.grounded = true;
    }

    /// The buttons that went down this tick. Call before `step`.
    pub fn pressed(&self, input: &Input) -> u16 {
        input.buttons & !self.held
    }

    pub fn is_locked(&self, tick: u32) -> bool {
        tick < self.locked_until
    }

    /// The height of the shoulder now.
    pub fn shoulder(&self) -> Fx {
        let breed = self.role.breed();
        if self.crouched {
            breed.shoulder_crouched
        } else {
            breed.shoulder
        }
    }

    pub fn step<G: Ground + ?Sized>(&mut self, ground: &G, input: &Input, tick: u32) {
        let breed = self.role.breed();
        let locked = self.is_locked(tick);
        let jump = self.pressed(input) & JUMP != 0;
        self.yaw = input.yaw;
        self.pitch = input.pitch;
        self.held = input.buttons;
        self.crouched = input.buttons & CROUCH != 0;

        let moving = !locked && (input.strafe != 0 || input.forward != 0);
        self.spend_stamina(moving && input.buttons & SPRINT != 0);
        let speed = if self.crouched {
            breed.crouch
        } else if self.sprinting {
            fx::mul(breed.sprint, self.sprint_scale)
        } else {
            breed.walk
        };

        if locked {
            (self.vx, self.vz) = (0, 0);
        } else {
            let want = if moving { wish(input, speed) } else { (0, 0) };
            let accel = if self.grounded {
                tune::GROUND_ACCEL
            } else {
                tune::AIR_ACCEL
            };
            (self.vx, self.vz) = approach((self.vx, self.vz), want, per_tick(accel));
            if jump && self.grounded {
                self.vy = tune::JUMP_SPEED;
                self.grounded = false;
            }
        }
        if !self.grounded {
            self.vy -= per_tick(tune::GRAVITY);
        }

        let (wanted_x, wanted_z) = (self.x + per_tick(self.vx), self.z + per_tick(self.vz));
        // where the feet will be this tick, so a jump clears a wall on the
        // tick its height allows
        let feet = self.y + per_tick(self.vy);
        let mover = match self.role {
            Role::Sheepdog => Mover::Sheepdog { feet },
            Role::Wolf => Mover::Wolf { feet },
        };
        let (x, z) = ground.push_out_as(wanted_x, wanted_z, breed.radius, mover);
        let (push_x, push_z) = at_most(x - wanted_x, z - wanted_z, per_tick(tune::WALL_EASE));
        let (x, z) = (wanted_x + push_x, wanted_z + push_z);
        if (x, z) != (wanted_x, wanted_z) {
            // a wall took some of the step, so it takes the same speed; it
            // gives none, however far it moves the body
            let moved = (
                (x - self.x) * tune::TICKS_PER_SECOND,
                (z - self.z) * tune::TICKS_PER_SECOND,
            );
            (self.vx, self.vz) = at_most(moved.0, moved.1, approx_len(self.vx, self.vz));
        }
        (self.x, self.z) = (x, z);

        self.y += per_tick(self.vy);
        let floor = ground.height(x, z);
        if self.grounded && self.y - floor > tune::STEP_DOWN {
            self.grounded = false;
        } else if self.grounded || self.y <= floor {
            self.y = floor;
            self.vy = 0;
            self.grounded = true;
        }
    }

    /// Take back this share of the step from `from`, which the body took
    /// this tick: it can only give ground it has just covered, so it never
    /// lands in a wall.
    pub fn held_back<G: Ground + ?Sized>(&mut self, ground: &G, from: (Fx, Fx), share: Fx) {
        if share == 0 {
            return;
        }
        self.x -= fx::mul(self.x - from.0, share);
        self.z -= fx::mul(self.z - from.1, share);
        if self.grounded {
            self.y = ground.height(self.x, self.z);
        }
    }

    /// Run the bar down at a sprint and up otherwise.
    fn spend_stamina(&mut self, wants_sprint: bool) {
        if self.winded && self.stamina >= tune::WINDED_UNTIL {
            self.winded = false;
        }
        self.sprinting = wants_sprint && !self.crouched && !self.winded;
        if self.sprinting {
            self.stamina -= tune::REFILL_TICKS;
            if self.stamina <= 0 {
                self.stamina = 0;
                self.winded = true;
            }
        } else {
            self.stamina = (self.stamina + tune::SPRINT_TICKS).min(tune::STAMINA_FULL);
        }
    }
}

/// The velocity an input asks for at this speed. A diagonal is no faster.
fn wish(input: &Input, speed: Fx) -> (Fx, Fx) {
    let (strafe, forward) = (
        fx::from_int(input.strafe as i32),
        fx::from_int(input.forward as i32),
    );
    let reach = fx::length(strafe, forward).max(fx::from_int(127));
    let (s, c) = (fx::sin(input.yaw as i32), fx::cos(input.yaw as i32));
    let x = fx::mul(forward, s) + fx::mul(strafe, c);
    let z = fx::mul(strafe, s) - fx::mul(forward, c);
    let scaled = |v: Fx| (v as i64 * speed as i64 / reach as i64) as Fx;
    (scaled(x), scaled(z))
}

/// Move a velocity toward a target by at most `rate`.
fn approach(from: (Fx, Fx), to: (Fx, Fx), rate: Fx) -> (Fx, Fx) {
    let (dx, dz) = (to.0 - from.0, to.1 - from.1);
    let d = fx::length(dx, dz);
    if d <= rate {
        return to;
    }
    let scale = fx::div(rate, d);
    (from.0 + fx::mul(dx, scale), from.1 + fx::mul(dz, scale))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::{from_int, ONE, QUARTER_TURN};
    use crate::ground::{FlatField, Rect, WalledField};
    use crate::map::table;
    use crate::map::walls::WallKind;
    use crate::map::Map;
    use crate::tune::{milli, seconds, SPRINT_TICKS, STAMINA_FULL};

    const FORWARD: Input = Input {
        strafe: 0,
        forward: 127,
        yaw: 0,
        pitch: 0,
        buttons: 0,
    };

    fn dog() -> Body {
        Body::new(Role::Sheepdog, 0, 0)
    }

    fn run(body: &mut Body, input: Input, ticks: i32) {
        for t in 0..ticks {
            body.step(&FlatField, &input, t as u32);
        }
    }

    /// The distance one tick covers: speed in Q16.16 over 60.
    fn stride(body: &mut Body, input: Input) -> Fx {
        let (x, z) = (body.x, body.z);
        body.step(&FlatField, &input, 0);
        fx::length(body.x - x, body.z - z)
    }

    #[test]
    fn walking_settles_at_the_contract_speed_for_each_team() {
        // 9 m/s is 589824 units a second, 9830 a tick, and 8.4 m/s is 9175
        let mut d = dog();
        run(&mut d, FORWARD, 30);
        assert_eq!(stride(&mut d, FORWARD), 9830);
        let mut w = Body::new(Role::Wolf, 0, 0);
        run(&mut w, FORWARD, 30);
        assert_eq!(stride(&mut w, FORWARD), 9175);
    }

    #[test]
    fn a_body_held_back_gives_up_that_share_of_its_step_and_no_more() {
        let mut body = dog();
        (body.x, body.z) = (from_int(4), from_int(2));
        body.held_back(&FlatField, (0, 0), fx::HALF);
        assert_eq!((body.x, body.z), (from_int(2), from_int(1)));
        body.held_back(&FlatField, (0, 0), 0);
        assert_eq!((body.x, body.z), (from_int(2), from_int(1)));
    }

    #[test]
    fn a_body_takes_about_a_tenth_of_a_second_to_reach_its_walk() {
        // 80 m/s^2 over 60 ticks is 1.333 m/s a tick; 9 / 1.333 = 6.75, so
        // the seventh tick arrives
        let mut d = dog();
        run(&mut d, FORWARD, 6);
        assert!(d.vz.abs() < tune::SHEEPDOG.walk, "{}", d.vz);
        run(&mut d, FORWARD, 1);
        assert_eq!(d.vz, -tune::SHEEPDOG.walk);
    }

    #[test]
    fn yaw_zero_looks_along_minus_z_and_a_quarter_turn_along_plus_x() {
        let mut d = dog();
        run(&mut d, FORWARD, 30);
        assert!(d.z < 0 && d.x == 0, "{} {}", d.x, d.z);
        let mut e = dog();
        run(
            &mut e,
            Input {
                yaw: QUARTER_TURN as u16,
                ..FORWARD
            },
            30,
        );
        assert!(e.x > 0 && e.z == 0, "{} {}", e.x, e.z);
    }

    #[test]
    fn strafing_right_at_yaw_zero_moves_along_plus_x() {
        let mut d = dog();
        run(
            &mut d,
            Input {
                strafe: 127,
                forward: 0,
                ..FORWARD
            },
            30,
        );
        assert!(d.x > 0 && d.z == 0, "{} {}", d.x, d.z);
    }

    #[test]
    fn a_diagonal_is_no_faster_than_straight_ahead() {
        let mut d = dog();
        run(
            &mut d,
            Input {
                strafe: 127,
                forward: 127,
                ..FORWARD
            },
            30,
        );
        let speed = fx::length(d.vx, d.vz);
        // the sine is good to about 0.002, which is what is left here
        assert!((speed - tune::SHEEPDOG.walk).abs() < ONE / 100, "{speed}");
    }

    #[test]
    fn sprinting_is_faster_and_a_full_bar_lasts_six_seconds() {
        let sprint = Input {
            buttons: SPRINT,
            ..FORWARD
        };
        let mut d = dog();
        run(&mut d, sprint, 30);
        // 16 m/s is 1048576 units a second, 17476 a tick
        assert_eq!(stride(&mut d, sprint), 17476);
        // 31 of the 360 ticks are spent
        assert_eq!(d.stamina, STAMINA_FULL - 31 * tune::REFILL_TICKS);
        run(&mut d, sprint, 328);
        assert!(d.sprinting && !d.winded);
        assert_eq!(d.stamina, tune::REFILL_TICKS);
        run(&mut d, sprint, 1);
        assert_eq!(d.stamina, 0);
        assert!(d.winded);
    }

    #[test]
    fn a_winded_body_walks_until_the_bar_refills_a_fifth() {
        let sprint = Input {
            buttons: SPRINT,
            ..FORWARD
        };
        let mut d = dog();
        d.stamina = tune::REFILL_TICKS; // one tick left
        run(&mut d, sprint, 1);
        assert!(d.winded);
        // a fifth of five seconds is 60 ticks, and holding the key does
        // nothing until then
        run(&mut d, sprint, 60);
        assert_eq!(d.stamina, STAMINA_FULL / 5);
        assert!(!d.sprinting && d.winded);
        run(&mut d, sprint, 1);
        assert!(d.sprinting && !d.winded);
    }

    #[test]
    fn the_bar_refills_in_five_seconds_at_rest() {
        let mut d = dog();
        d.stamina = 0;
        run(&mut d, Input::default(), seconds(5) - 1);
        assert_eq!(d.stamina, STAMINA_FULL - SPRINT_TICKS);
        run(&mut d, Input::default(), 1);
        assert_eq!(d.stamina, STAMINA_FULL);
    }

    #[test]
    fn holding_sprint_while_standing_still_spends_nothing() {
        let mut d = dog();
        run(
            &mut d,
            Input {
                buttons: SPRINT,
                ..Input::default()
            },
            60,
        );
        assert!(!d.sprinting);
        assert_eq!(d.stamina, STAMINA_FULL);
    }

    #[test]
    fn crouched_movement_is_slower_and_cancels_a_sprint() {
        let crouch = Input {
            buttons: CROUCH | SPRINT,
            ..FORWARD
        };
        let mut d = dog();
        run(&mut d, crouch, 30);
        // 4.4 m/s is 288358 units a second, 4805 a tick
        assert_eq!(stride(&mut d, crouch), 4805);
        assert!(d.crouched && !d.sprinting);
        assert_eq!(d.stamina, STAMINA_FULL);
    }

    #[test]
    fn crouching_lowers_the_shoulder_to_the_contract_height() {
        let mut w = Body::new(Role::Wolf, 0, 0);
        assert_eq!(w.shoulder(), milli(800));
        run(
            &mut w,
            Input {
                buttons: CROUCH,
                ..Input::default()
            },
            1,
        );
        assert_eq!(w.shoulder(), milli(600));
    }

    #[test]
    fn a_jump_peaks_past_a_wall_top_and_lands_after_about_three_quarters_of_a_second() {
        // v = 7.6 - k/3 m/s on tick k; the peak is after tick 22, at
        // (7.6 * 22 - 22 * 23 / 6) / 60 = 1.3811 m. Back at the ground when
        // 7.6 n = n (n + 1) / 6, so n = 45.
        let mut d = dog();
        d.step(
            &FlatField,
            &Input {
                buttons: JUMP,
                ..Input::default()
            },
            0,
        );
        let mut peak = d.y;
        let mut flight = 1;
        while !d.grounded {
            d.step(&FlatField, &Input::default(), 0);
            peak = peak.max(d.y);
            flight += 1;
            assert!(flight < 100, "never landed");
        }
        assert!((peak - milli(1381)).abs() < milli(5), "{peak}");
        assert!((45..=46).contains(&flight), "{flight}");
        assert_eq!(d.y, 0);
        assert!(peak >= table::INNER_WALL_HEIGHT + milli(200));
    }

    #[test]
    fn a_held_jump_does_not_jump_again_and_there_is_no_jump_in_the_air() {
        let jump = Input {
            buttons: JUMP,
            ..Input::default()
        };
        let mut d = dog();
        run(&mut d, jump, 10);
        assert!(d.vy > 0 && d.vy < tune::JUMP_SPEED);
        // released and pressed again, still in the air
        run(&mut d, Input::default(), 1);
        let rising = d.vy;
        run(&mut d, jump, 1);
        assert!(d.vy < rising);
        run(&mut d, jump, 100);
        assert_eq!(
            (d.y, d.vy),
            (0, 0),
            "holding the key through a landing stays down"
        );
    }

    #[test]
    fn a_locked_body_stands_still_until_its_tick_and_still_looks_around() {
        let mut w = Body::new(Role::Wolf, 0, 0);
        w.locked_until = 60;
        let wants = Input {
            yaw: 123,
            pitch: 45,
            buttons: JUMP | SPRINT,
            ..FORWARD
        };
        for t in 0..60 {
            w.step(&FlatField, &wants, t);
        }
        assert_eq!((w.x, w.z, w.y), (0, 0, 0));
        assert_eq!((w.yaw, w.pitch), (123, 45));
        assert!(!w.sprinting);
        assert_eq!(w.stamina, STAMINA_FULL);
        // tick 60 is the first it may move
        w.step(&FlatField, &wants, 60);
        assert!(w.z < 0, "{}", w.z);
    }

    #[test]
    fn a_lock_drops_a_running_bodys_speed_at_once() {
        let mut d = dog();
        run(&mut d, FORWARD, 30);
        d.locked_until = 1000;
        let z = d.z;
        d.step(&FlatField, &FORWARD, 30);
        assert_eq!((d.vx, d.vz), (0, 0));
        assert_eq!(d.z, z);
    }

    #[test]
    fn a_wall_stops_a_body_and_takes_its_speed() {
        let ground = WalledField {
            walls: vec![Rect {
                min_x: from_int(-5),
                min_z: from_int(-3),
                max_x: from_int(5),
                max_z: from_int(-2),
            }],
            dividers: vec![],
            fold: None,
        };
        let mut d = dog();
        for t in 0..120 {
            d.step(&ground, &FORWARD, t);
        }
        // the wall's face at z = -2, plus the dog's radius
        assert_eq!(d.z, from_int(-2) + tune::SHEEPDOG.radius);
        assert_eq!(d.vz, 0);
    }

    /// A slope of 1 in 10 up along -z, then a drop to 0 past z = -20.
    struct Slope;

    impl Ground for Slope {
        fn height(&self, _x: Fx, z: Fx) -> Fx {
            if z < from_int(-20) {
                0
            } else {
                -z / 10
            }
        }

        fn push_out(&self, x: Fx, z: Fx, _radius: Fx) -> (Fx, Fx) {
            (x, z)
        }
    }

    #[test]
    fn a_body_walks_up_a_slope_and_falls_off_an_edge() {
        let mut d = dog();
        for t in 0..120 {
            d.step(&Slope, &FORWARD, t);
        }
        // two seconds at 9 m/s is about 17.7 m along, so 1.77 m up
        assert_eq!(d.y, -d.z / 10);
        assert!(d.grounded && d.y > ONE / 2);
        while d.z >= from_int(-20) {
            d.step(&Slope, &FORWARD, 0);
        }
        // 2 m above a floor of 0: the fall starts at the edge
        assert!(!d.grounded);
        assert!(d.y > ONE);
        for t in 0..120 {
            d.step(&Slope, &FORWARD, t);
        }
        assert!(d.grounded);
        assert_eq!(d.y, 0);
    }

    const EAST: Input = Input {
        strafe: 0,
        forward: 127,
        yaw: QUARTER_TURN as u16,
        pitch: 0,
        buttons: 0,
    };

    /// The middle of the longest run of inner wall, and a body 5 m west of
    /// it, on the ground.
    fn before_a_wall(map: &Map, role: Role) -> (Fx, Body) {
        let wall = map
            .walls
            .iter()
            .filter(|w| w.kind == WallKind::Inner)
            .max_by_key(|w| (w.b.1 - w.a.1).abs())
            .expect("inner walls");
        let (x, z) = ((wall.a.0 + wall.b.0) / 2, (wall.a.1 + wall.b.1) / 2);
        let mut body = Body::new(role, x - from_int(5), z);
        body.settle(map);
        (x, body)
    }

    #[test]
    fn a_walk_stops_at_an_inner_wall_of_the_map_and_a_running_jump_clears_it() {
        let map = Map::generate(1);
        for role in [Role::Sheepdog, Role::Wolf] {
            let (line, mut b) = before_a_wall(&map, role);
            for t in 0..seconds(3) {
                b.step(&map, &EAST, t as u32);
            }
            assert!(b.x < line, "{}", fx::floor(b.x - line));

            // at a sprint, leaving the ground 3 m from the line, about
            // 2 m short of the stone
            let sprint = Input {
                buttons: SPRINT,
                ..EAST
            };
            let (line, mut b) = before_a_wall(&map, role);
            let mut t = 0;
            while b.x < line - from_int(3) {
                b.step(&map, &sprint, t);
                t += 1;
            }
            let leap = Input {
                buttons: SPRINT | JUMP,
                ..EAST
            };
            b.step(&map, &leap, t);
            for k in 0..seconds(1) {
                b.step(&map, &sprint, t + 1 + k as u32);
                // never kicked: a wall takes speed and gives none
                assert!(fx::length(b.vx, b.vz) <= role.breed().sprint + milli(50));
            }
            assert!(b.x > line, "{role:?}");
            assert!(b.grounded);
        }
    }

    #[test]
    fn a_zoomies_sprint_stops_at_every_wall_of_the_map() {
        // 175% of 16 m/s is 28 m/s, 0.47 m a tick, on the ground and flat
        // out, at seven spots along each side of every wall of three maps
        for seed in 1..=3 {
            let map = Map::generate(seed);
            let mut runs = 0;
            for wall in &map.walls {
                let (dx, dz) = (wall.b.0 - wall.a.0, wall.b.1 - wall.a.1);
                let long = fx::length(dx, dz);
                if long < from_int(8) {
                    continue;
                }
                let (nx, nz) = (fx::div(-dz, long), fx::div(dx, long));
                for spot in 1..8 {
                    let at = (wall.a.0 + dx / 8 * spot, wall.a.1 + dz / 8 * spot);
                    for side in [ONE, -ONE] {
                        let (ox, oz) = (fx::mul(nx, side), fx::mul(nz, side));
                        let mut b = Body::new(Role::Sheepdog, at.0 + ox * 6, at.1 + oz * 6);
                        let inside = map.push_out(b.x, b.z, tune::SHEEPDOG.radius) == (b.x, b.z);
                        if !inside || !map.inside(b.x, b.z) {
                            continue;
                        }
                        runs += 1;
                        b.settle(&map);
                        b.sprint_scale = crate::rules::powerups::zoomies_sprint(ONE);
                        (b.vx, b.vz) = (-fx::mul(ox, milli(28_000)), -fx::mul(oz, milli(28_000)));
                        let dash = Input {
                            yaw: crate::game::aim::yaw_toward(-ox, -oz),
                            buttons: SPRINT,
                            ..FORWARD
                        };
                        for t in 0..40 {
                            b.step(&map, &dash, t);
                        }
                        let past = (b.x - at.0) as i64 * ox as i64 + (b.z - at.1) as i64 * oz as i64;
                        assert!(past > 0, "seed {seed} {wall:?} spot {spot}");
                    }
                }
            }
            assert!(runs > 100, "{runs}");
        }
    }

    #[test]
    fn a_sheepdog_walks_through_the_gap_of_the_fold_and_a_wolf_is_held_out() {
        let map = Map::generate(1);
        let (x, z) = map.fold.gap.center;
        let walk = |role: Role| {
            let mut b = Body::new(role, x - from_int(4), z);
            b.settle(&map);
            for t in 0..seconds(3) {
                b.step(&map, &EAST, t as u32);
            }
            map.in_fold(b.x, b.z)
        };
        assert!(walk(Role::Sheepdog));
        assert!(!walk(Role::Wolf));
    }

    #[test]
    fn a_body_dropped_on_a_wall_top_is_eased_off_and_not_thrown() {
        // standing right on the line of a wall: the map wants it 0.86 m away
        // at once, and it goes at 30 m/s, half a meter a tick
        let map = Map::generate(1);
        let (line, mut b) = before_a_wall(&map, Role::Sheepdog);
        b.x = line;
        let from = (b.x, b.z);
        b.step(&map, &Input::default(), 0);
        // the whole move: on the line itself the way out can be any way
        let moved = fx::length(b.x - from.0, b.z - from.1);
        assert!(moved > milli(400) && moved <= milli(501), "{moved}");
        assert_eq!((b.vx, b.vz), (0, 0));
    }
}
