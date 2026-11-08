//! Bots: the seats no person holds (PLAN 15). A bot runs inside `Match` on
//! every browser and through every rollback replay, so all it keeps is
//! `Brain`, which sits on `Player`, is copied with the match and goes into the
//! checksum (FL-1).
//!
//! A bot decides a few times a second, on a tick fixed by its index, and in
//! between it only steers. It decides from `Match::snapshot_for`: the same
//! `Snapshot` a person on that team gets, so FL-7's one view rules it too. A
//! Sheepdog bot takes no Wolf from that list unless the Wolf gives itself
//! away (`gives_itself_away`), as a Wolf hiding in the flock doesn't.
//!
//! Sheepdogs drive the sheep field by field, each dog keeping to a share of
//! the map's width, nearest field to the Fold first. Wolves stay
//! with the sheep, eat when no Sheepdog is near, and leave.

use super::snapshot::{Action, BodyRow, SheepRow, Snapshot};
use super::{aim, Match};
use crate::body::{Body, Input, Role, ACT, CROUCH, DISCARD, JUMP, PRIMARY, SECONDARY, SPRINT};
use crate::fx::{self, Fx};
use crate::ground::{Ground, FOLD_FIELD};
use crate::rules::information;
use crate::rules::mounds::MoundLook;
use crate::rules::tune::{self, Powerup};

/// How a bot means to move.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
enum Pace {
    #[default]
    Walk,
    Crouch,
    Run,
}

/// What a bot points its crosshair at.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
enum Look {
    /// Where it's going.
    #[default]
    Ahead,
    Sheep(u16),
    /// A body it has a reason to follow, by index.
    Body(u8),
    /// Something that stays put: a mound, a mouth, a carcass.
    Spot(Fx, Fx),
}

/// What a bot means to do until it next decides.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
struct Order {
    go: Option<(Fx, Fx)>,
    pace: Pace,
    look: Look,
    /// Pressed once, when the crosshair is on `look`.
    press: u16,
    /// Held while the crosshair is on `look`.
    hold: u16,
}

/// All a bot keeps between ticks.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Brain {
    order: Order,
    /// The Wolf this Sheepdog is after, by index, and for how many more
    /// ticks it can keep its eye on it.
    quarry: u8,
    quarry_left: i32,
    /// Where the bot last decided, to notice it isn't getting anywhere.
    last_at: (Fx, Fx),
    stuck: u8,
    /// Ticks left of stepping sideways around what stopped it, and which way.
    dodge: i32,
    dodge_right: bool,
    /// The sheep this Sheepdog is driving, and ticks left to do it.
    job: u16,
    job_left: i32,
    /// The field it's in. Out of that field, the job is done.
    job_field: u8,
    /// One more than the sheep last given up on, or 0.
    skip: u16,
    /// When this Sheepdog last barked, in ticks.
    barked: u32,
    /// Where this Wolf last ate.
    last_meal: Option<(Fx, Fx)>,
    ate_at: u32,
    /// How many ticks it has held a powerup that won't go off.
    held_for: i32,
}

/// A direction as a unit vector, or nothing for no length.
fn unit(x: Fx, z: Fx) -> Option<(Fx, Fx)> {
    let d = fx::length(x, z);
    (d > 0).then(|| (fx::div(x, d), fx::div(z, d)))
}

/// A share of full speed, -1 to 1, as an `Input` axis.
fn stick(share: Fx) -> i8 {
    fx::round(share * 127).clamp(-127, 127) as i8
}

fn apart(a: (Fx, Fx), b: (Fx, Fx)) -> Fx {
    fx::length(a.0 - b.0, a.1 - b.1)
}

fn dot(a: (Fx, Fx), b: (Fx, Fx)) -> i64 {
    a.0 as i64 * b.0 as i64 + a.1 as i64 * b.1 as i64
}

/// Whether a Sheepdog at this distance can tell this body isn't a sheep.
/// Everything here is on a person's screen: a tall back, a run no sheep
/// makes, a Wolf at its meal, and just after a bark, something that didn't
/// bolt. A crouched Wolf keeping pace with the flock shows none of them, at
/// any distance, so this is the only way a Wolf's row may reach a bot's plan.
fn gives_itself_away(row: &BodyRow, far: Fx, just_barked: bool) -> bool {
    let unmoved = just_barked && row.speed < tune::BOT_MOVING && far <= tune::BOT_TELL_BARK;
    unmoved
        || match row.action {
            Action::Snapping | Action::Lunging => far <= tune::BOT_TELL_EATING,
            Action::Digging => far <= tune::BOT_TELL_RUNNING,
            // a Wolf in the Sheep suit passes for a sheep, standing or running
            Action::Moving if row.suited => false,
            Action::Moving if row.sprinting => far <= tune::BOT_TELL_RUNNING,
            Action::Moving => !row.crouched && far <= tune::BOT_TELL_TALL,
        }
}

impl Match {
    /// What a seat with no person in it does this tick. An empty seat in the
    /// lobby and a seat its player left both come here. It reads `Match`
    /// state alone.
    pub(super) fn bot_input(&mut self, k: usize) -> Input {
        let turn = self.world.tick + k as u32 * tune::BOT_THINK_STAGGER;
        if turn.is_multiple_of(tune::BOT_THINK_TICKS) {
            self.think(k);
        }
        self.steer(k)
    }

    fn think(&mut self, k: usize) {
        let Some(snap) = self.snapshot_for(self.players[k].seat as usize) else {
            return;
        };
        let body = self.world.bodies[k];
        let mut brain = self.players[k].brain;
        if !snap.me.alive || snap.me.stunned {
            self.players[k].brain.order = Order::default();
            return;
        }
        let order = match body.role {
            Role::Sheepdog => self.sheepdog_order(k, &snap, &mut brain),
            Role::Wolf => self.wolf_order(k, &snap, &mut brain),
        };
        let plant = self.players[k].slot.held() == Some(Powerup::Bone)
            && order.pace != Pace::Run
            && self.bone_spot(&body).is_some();
        let order = if plant {
            Order {
                hold: SECONDARY,
                ..Order::default()
            }
        } else {
            brain.spend_powerup(&snap, order)
        };
        brain.note_progress(&body, &order);
        brain.order = order;
        self.players[k].brain = brain;
    }

    /// The next stop on the way from `from` to `to`: straight there in one
    /// field, otherwise through the gap in the next wall that costs least.
    fn route(&self, from: (Fx, Fx), to: (Fx, Fx)) -> (Fx, Fx) {
        let (here, there) = (self.map.field(from.0, from.1), self.map.field(to.0, to.1));
        if here == there {
            return to;
        }
        let up = there > here;
        let wall = if up { here } else { here - 1 };
        let cost = |c: (Fx, Fx)| apart(from, c) as i64 + apart(c, to) as i64;
        let wall_gaps = self.map.every_gap().filter(|g| g.wall == wall);
        let Some(gap) = wall_gaps.min_by_key(|g| cost(g.center)) else {
            return to;
        };
        let way = if up {
            tune::BOT_DOOR_PAST
        } else {
            -tune::BOT_DOOR_PAST
        };
        let in_line = (from.1 - gap.center.1).abs() <= tune::BOT_DOOR_LINE;
        (gap.center.0 + if in_line { way } else { -way }, gap.center.1)
    }

    /// Where a sheep in this field has to go next.
    fn sheep_goal(&self, field: u8, x: Fx, z: Fx) -> (Fx, Fx) {
        let gaps = self.map.every_gap().filter(|g| g.wall == field);
        let nearest = gaps.min_by_key(|g| apart(g.center, (x, z)));
        let center = nearest.map_or(self.map.fold.center, |g| g.center);
        (center.0 + tune::BOT_DOOR_PAST, center.1)
    }

    // ---- Sheepdogs ----

    fn sheepdog_order(&self, k: usize, snap: &Snapshot, brain: &mut Brain) -> Order {
        let me = (self.world.bodies[k].x, self.world.bodies[k].z);
        self.watch_for_wolves(me, snap, brain);
        let chased = self.chase(me, snap, brain);
        let dealt_with = chased.or_else(|| self.check_carcass(me));
        if let Some(order) = dealt_with.or_else(|| self.dig_near(me, snap, Pace::Run)) {
            return order;
        }
        self.herd(k, me, snap, brain)
    }

    /// Any Wolf that gives itself away becomes this bot's quarry.
    fn watch_for_wolves(&self, me: (Fx, Fx), snap: &Snapshot, brain: &mut Brain) {
        brain.quarry_left = (brain.quarry_left - tune::BOT_THINK_TICKS as i32).max(0);
        let wolves = snap.bodies.iter().filter(|b| b.role == Role::Wolf);
        let just_barked = brain.barked > 0 && snap.tick <= brain.barked + tune::BOT_BARK_LOOK_TICKS;
        let seen = wolves.filter(|b| gives_itself_away(b, apart(me, (b.x, b.z)), just_barked));
        if let Some(row) = seen.min_by_key(|b| apart(me, (b.x, b.z))) {
            if let Some(j) = self.index_of(row.seat as usize) {
                brain.quarry = j as u8;
                brain.quarry_left = tune::BOT_QUARRY_TICKS;
            }
        }
    }

    /// After the quarry, as far as this Sheepdog's own screen shows it: a
    /// Wolf that cloaks drops out of the snapshot, and the chase with it.
    fn chase(&self, me: (Fx, Fx), snap: &Snapshot, brain: &mut Brain) -> Option<Order> {
        if brain.quarry_left == 0 {
            return None;
        }
        let seat = self.players[brain.quarry as usize].seat;
        let shown = snap.bodies.iter().find(|b| b.seat == seat);
        let Some(quarry) = shown.filter(|_| !self.world.bodies[brain.quarry as usize].out) else {
            brain.quarry_left = 0;
            return None;
        };
        Some(Order {
            go: Some(self.route(me, (quarry.x, quarry.z))),
            pace: Pace::Run,
            look: Look::Body(brain.quarry),
            ..Order::default()
        })
    }

    /// A carcass nearby that no Sheepdog has found gets a look.
    fn check_carcass(&self, me: (Fx, Fx)) -> Option<Order> {
        let fresh = self.carcasses.iter().filter(|c| !c.carcass.found());
        let near = fresh.filter(|c| apart(me, (c.x, c.z)) <= tune::BOT_CARCASS_REACH);
        let nearest = near.min_by_key(|c| apart(me, (c.x, c.z)))?;
        Some(Order {
            look: Look::Spot(nearest.x, nearest.z),
            ..Order::default()
        })
    }

    /// With no powerup held, a fresh mound nearby is worth the walk and the dig.
    fn dig_near(&self, me: (Fx, Fx), snap: &Snapshot, pace: Pace) -> Option<Order> {
        if snap.me.held.is_some() {
            return None;
        }
        let looks = snap.view.mounds.iter().zip(&snap.mound_spots);
        let fresh = looks.filter(|(look, _)| **look == MoundLook::Fresh);
        let at = |(_, m): &(&MoundLook, &[Fx; 2])| (m[0], m[1]);
        let nearest = fresh.min_by_key(|m| apart(me, at(m)))?;
        let spot = at(&nearest);
        let far = apart(me, spot);
        if far > tune::BOT_MOUND_REACH {
            return None;
        }
        if far <= tune::BOT_DIG_FROM {
            return Some(Order {
                look: Look::Spot(spot.0, spot.1),
                hold: ACT,
                pace,
                ..Order::default()
            });
        }
        Some(Order {
            go: Some(self.route(me, spot)),
            pace,
            ..Order::default()
        })
    }

    /// The sheep this bot works next: the one with farthest to go in the
    /// field nearest the Fold with more in it than a Sheepdog stays for, in
    /// this bot's share of the map's width first, of the sheep in sight.
    fn rearmost(&self, k: usize, snap: &Snapshot, skip: u16) -> Option<u16> {
        let seat = self.players[k].seat;
        let (mut team, mut mine) = (0, 0);
        for dog in snap.bodies.iter().filter(|b| b.role == Role::Sheepdog) {
            mine += (dog.seat < seat) as i64;
            team += 1;
        }
        let width = self.map.width() as i64;
        let share = |z: Fx| ((z as i64 * team) / width).clamp(0, team - 1);
        let sheep = &self.world.flock.sheep;
        let field_of = |r: &SheepRow| sheep[r.id as usize].field as usize;
        let free = || {
            snap.sheep
                .iter()
                .filter(|r| !r.folded && field_of(r) < FOLD_FIELD as usize)
        };
        let (mut in_share, mut in_all) = ([0; FOLD_FIELD as usize], [0; FOLD_FIELD as usize]);
        for row in free() {
            in_all[field_of(row)] += 1;
            in_share[field_of(row)] += (share(row.z) == mine) as i32;
        }
        let worth = |n: &i32| *n > tune::BOT_LEAVE_BEHIND;
        let highest = |counts: &[i32]| counts.iter().rposition(worth);
        let (field, by_share) = match (highest(&in_share), highest(&in_all)) {
            (Some(f), _) => (f, true),
            (None, Some(f)) => (f, false),
            (None, None) => (in_all.iter().rposition(|n| *n > 0)?, false),
        };
        let chosen =
            |r: &&SheepRow| field_of(r) == field && (!by_share || share(r.z) == mine) && r.id != skip;
        let to_go = |r: &&SheepRow| {
            let goal = self.sheep_goal(field as u8, r.x, r.z);
            apart((r.x, r.z), goal)
        };
        free().filter(chosen).max_by_key(to_go).map(|r| r.id)
    }

    /// Drive a sheep from the back of the flock all the way to its gap, the
    /// sheep it meets on the way swept along with it, then go back for the
    /// next. A sheep that won't come, likely pinned on a wall, gets left.
    fn herd(&self, k: usize, me: (Fx, Fx), snap: &Snapshot, brain: &mut Brain) -> Order {
        let sheep = &self.world.flock.sheep;
        let beat = tune::BOT_THINK_TICKS as i32;
        brain.job_left = (brain.job_left - beat).max(0);
        let job = &sheep[brain.job as usize];
        let on_job = !job.eaten && !job.folded && job.field == brain.job_field;
        if brain.job_left == 0 && on_job {
            brain.skip = brain.job + 1;
        }
        if brain.job_left == 0 || !on_job {
            let Some(id) = self.rearmost(k, snap, brain.skip.wrapping_sub(1)) else {
                // nothing in sight but the Fold: look down the hill
                let middle = (self.map.fold.center.0 / 2, self.map.width() / 2);
                return Order {
                    go: Some(self.route(me, middle)),
                    pace: Pace::Run,
                    ..Order::default()
                };
            };
            brain.job = id;
            brain.job_field = sheep[id as usize].field;
            brain.job_left = tune::BOT_JOB_TICKS;
        }
        let job = &sheep[brain.job as usize];
        let field = brain.job_field;
        let at = (job.x, job.z);
        let goal = self.sheep_goal(field, at.0, at.1);
        let behind = tune::BOT_PRESSURE;
        let Some(back) = unit(at.0 - goal.0, at.1 - goal.1) else {
            return Order::default();
        };
        let from_sheep = (me.0 - at.0, me.1 - at.1);
        if dot(from_sheep, back) < 0 {
            // on the wrong side: run wide round the sheep, not through them
            let side = (-back.1, back.0);
            let wide = behind + tune::BOT_FLANK;
            let reach = if dot(from_sheep, side) >= 0 { wide } else { -wide };
            let wide = self.post_behind(at, side, reach, field);
            return Order {
                go: Some(self.route(me, wide)),
                pace: Pace::Run,
                ..Order::default()
            };
        }
        let to = self.route(me, self.post_behind(at, back, behind, field));
        let way = (to.0 - me.0, to.1 - me.1);
        let in_the_way = |r: &SheepRow| {
            let off = (r.x - me.0, r.z - me.1);
            let near = off.0.abs() < tune::BOT_STAND_OFF && off.1.abs() < tune::BOT_STAND_OFF;
            let moving = r.speed > tune::BOT_MOVING;
            near && moving && !r.folded && fx::length(off.0, off.1) < tune::BOT_STAND_OFF && dot(off, way) > 0
        };
        let closing = snap.sheep.iter().any(in_the_way);
        let wait_over = snap.tick >= brain.barked + tune::BOT_BARK_TICKS;
        let bark = wait_over && apart(me, at) < behind + tune::BOT_BARK_REACH;
        if bark {
            brain.barked = snap.tick;
        }
        Order {
            go: (!closing).then_some(to),
            pace: if apart(me, to) > tune::BOT_RUN_FROM {
                Pace::Run
            } else {
                Pace::Walk
            },
            look: Look::Ahead,
            press: if bark { ACT } else { 0 },
            ..Order::default()
        }
    }

    /// The spot `behind` meters along `way` from `at`, or nearer when a wall
    /// comes first: past a wall a Sheepdog pushes nothing.
    fn post_behind(&self, at: (Fx, Fx), way: (Fx, Fx), behind: Fx, field: u8) -> (Fx, Fx) {
        let mut reach = behind;
        while reach.abs() > tune::BOT_ARRIVED {
            let spot = self.clear_spot((at.0 + fx::mul(way.0, reach), at.1 + fx::mul(way.1, reach)));
            if self.map.field(spot.0, spot.1) == field {
                return spot;
            }
            reach /= 2;
        }
        at
    }

    /// The nearest place to this one a body can stand.
    fn clear_spot(&self, at: (Fx, Fx)) -> (Fx, Fx) {
        self.map.push_out(at.0, at.1, tune::BOT_WALL_ROOM)
    }

    // ---- Wolves ----

    fn wolf_order(&self, k: usize, snap: &Snapshot, brain: &mut Brain) -> Order {
        let body = &self.world.bodies[k];
        let me = (body.x, body.z);
        let dogs = snap.bodies.iter().filter(|b| b.role == Role::Sheepdog);
        let nearest = dogs.min_by_key(|b| apart(me, (b.x, b.z)));
        let danger = nearest.map_or(Fx::MAX, |b| apart(me, (b.x, b.z)));
        if let Some(dog) = nearest.filter(|_| danger < tune::BOT_FLEE_REACH) {
            return self.flee(k, me, (dog.x, dog.z));
        }
        let pace = if danger < tune::BOT_HIDE_REACH {
            Pace::Crouch
        } else {
            Pace::Walk
        };
        let hungry = snap.me.eat_seconds == Some(0) && self.players[k].brain.waited_for_meal(snap.tick);
        if !hungry && danger >= tune::BOT_HIDE_REACH {
            if let Some(order) = self.dig_near(me, snap, Pace::Walk) {
                return order;
            }
        }
        let sheep = &self.world.flock.sheep;
        let by_last_meal = |r: &SheepRow| {
            let meal = brain.last_meal.filter(|_| !hungry);
            meal.is_some_and(|m| apart(m, (r.x, r.z)) < tune::BOT_MOVE_ON)
        };
        let free = snap
            .sheep
            .iter()
            .filter(|r| !r.folded && sheep[r.id as usize].field != FOLD_FIELD);
        let Some(prey) = free
            .filter(|r| !by_last_meal(r))
            .min_by_key(|r| apart(me, (r.x, r.z)))
        else {
            return Order {
                pace,
                ..Order::default()
            };
        };
        let at = (prey.x, prey.z);
        let far = apart(me, at);
        if hungry && danger >= tune::BOT_EAT_SAFE && far <= tune::BOT_EAT_FROM {
            brain.last_meal = Some(at);
            brain.ate_at = snap.tick;
            return Order {
                pace,
                look: Look::Sheep(prey.id),
                press: PRIMARY,
                ..Order::default()
            };
        }
        Order {
            go: (far > tune::BOT_MINGLE).then(|| self.route(me, at)),
            pace,
            ..Order::default()
        }
    }

    /// Away from a Sheepdog, down a tunnel if one is close enough and the
    /// tunnel cooldown is over.
    fn flee(&self, k: usize, me: (Fx, Fx), dog: (Fx, Fx)) -> Order {
        let ready = self.players[k].tunnel.fraction() == fx::ONE;
        let mouths = self.map.mouths();
        let mouth = mouths
            .iter()
            .filter(|m| ready && apart(me, **m) <= tune::BOT_MOUTH_REACH);
        // not past the Sheepdog: it has to be nearer this Wolf
        let open = mouth.filter(|m| apart(me, **m) < apart(dog, **m));
        if let Some(m) = open.min_by_key(|m| apart(me, **m)) {
            let close = apart(me, *m) <= tune::BOT_DIG_FROM;
            return Order {
                go: (!close).then(|| self.route(me, *m)),
                pace: Pace::Run,
                look: if close { Look::Spot(m.0, m.1) } else { Look::Ahead },
                press: if close { ACT } else { 0 },
                ..Order::default()
            };
        }
        let away = unit(me.0 - dog.0, me.1 - dog.1).unwrap_or((fx::ONE, 0));
        let far = (
            me.0 + away.0 * tune::BOT_FLEE_RUN,
            me.1 + away.1 * tune::BOT_FLEE_RUN,
        );
        Order {
            go: Some(self.clear_spot(far)),
            pace: Pace::Run,
            ..Order::default()
        }
    }

    // ---- Every tick ----

    /// One tick's `Input` from the bot's order: turn, walk, press.
    fn steer(&mut self, k: usize) -> Input {
        let body = self.world.bodies[k];
        let mut brain = self.players[k].brain;
        let mut order = brain.order;
        let me = (body.x, body.z);
        let target = match order.look {
            Look::Ahead => None,
            Look::Sheep(id) => {
                let s = &self.world.flock.sheep[id as usize];
                (!s.eaten).then_some((s.x, s.z))
            }
            Look::Body(j) => {
                let b = &self.world.bodies[j as usize];
                let cloaked = self.players[j as usize].slot.running(Powerup::InvisibilityCloak);
                let hidden = information::hidden(body.role, b.role, cloaked);
                (!b.out && !hidden).then_some((b.x, b.z))
            }
            Look::Spot(x, z) => Some((x, z)),
        };
        if let (Look::Body(_), Some(at)) = (order.look, target) {
            order.go = Some(self.route(me, at));
            if apart(me, at) <= tune::BOT_LUNGE_FROM && self.players[k].lunge.ready() {
                order.press |= PRIMARY;
            }
        }
        let goes = order.go.filter(|g| apart(me, *g) > tune::BOT_ARRIVED);
        let way = goes.and_then(|g| unit(g.0 - me.0, g.1 - me.1));
        let face = target.and_then(|t| unit(t.0 - me.0, t.1 - me.1)).or(way);
        let want = face.map_or(body.yaw, |(x, z)| aim::yaw_toward(x, z));
        let yaw = aim::turned_toward(body.yaw, want, tune::BOT_TURN);
        let pitch = target.map_or(aim::REST_PITCH, |t| {
            aim::pitch_toward(apart(me, t), self.map.height(t.0, t.1) - body.y)
        });
        let mut buttons = match order.pace {
            Pace::Crouch => CROUCH,
            Pace::Run => SPRINT,
            Pace::Walk => 0,
        };
        if aim::yaw_apart(yaw, want) <= tune::BOT_AIMED {
            buttons |= order.press | order.hold;
            brain.order.press = 0;
        }
        let (s, c) = (fx::sin(yaw as i32), fx::cos(yaw as i32));
        // how much of the way lies ahead and to the right of this yaw
        let (mut forward, mut strafe) = way.map_or((0, 0), |(x, z)| {
            (fx::mul(x, s) - fx::mul(z, c), fx::mul(x, c) + fx::mul(z, s))
        });
        if brain.dodge > 0 {
            if brain.dodge == tune::BOT_DODGE_TICKS {
                buttons |= JUMP;
            }
            brain.dodge -= 1;
            strafe = if brain.dodge_right { fx::ONE } else { -fx::ONE };
            forward = 0;
        }
        self.players[k].brain = brain;
        Input {
            strafe: stick(strafe),
            forward: stick(forward),
            yaw,
            pitch,
            buttons,
        }
    }
}

impl Brain {
    /// A person would not eat the moment the dial comes round: a bot takes
    /// its time too, looking for its moment.
    fn waited_for_meal(&self, now: u32) -> bool {
        self.last_meal.is_none() || now >= self.ate_at + tune::BOT_MEAL_EVERY_TICKS
    }

    /// A powerup held goes off at once, and one that can't, here, is thrown
    /// away after a while so the slot can take another.
    fn spend_powerup(&mut self, snap: &Snapshot, mut order: Order) -> Order {
        if snap.me.held.is_none() {
            self.held_for = 0;
            return order;
        }
        self.held_for += tune::BOT_THINK_TICKS as i32;
        let spent = self.held_for >= tune::BOT_HOLD_TICKS;
        order.press |= if spent { DISCARD } else { SECONDARY };
        order
    }

    /// A bot that means to move and hasn't sidesteps what's in its way.
    fn note_progress(&mut self, body: &Body, order: &Order) {
        let moved = apart(self.last_at, (body.x, body.z));
        self.last_at = (body.x, body.z);
        if order.go.is_none() || moved >= tune::BOT_STUCK_MOVE {
            self.stuck = 0;
            return;
        }
        self.stuck += 1;
        if self.stuck >= tune::BOT_STUCK_THINKS {
            self.stuck = 0;
            self.dodge = tune::BOT_DODGE_TICKS;
            self.dodge_right = !self.dodge_right;
        }
    }
}

#[cfg(test)]
#[path = "bot_tests.rs"]
mod tests;
