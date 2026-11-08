//! One match: `World`, the `Map` and `rules/` joined into one deterministic
//! tick, built from a `MatchStart`. Every browser runs the same `Match` and
//! only inputs cross the wire (FL-1). `Clone` copies flat Vecs and shares the
//! map behind an `Arc`, since a map never changes, so a rollback keeps a copy
//! a tick.
//!
//! What the keys mean (R1.118). The rules here read each button's fresh press:
//!
//! | Button | Sheepdog | Wolf |
//! | --- | --- | --- |
//! | Left click, `PRIMARY` | Lunge | eat the sheep under the crosshair |
//! | Right click, `SECONDARY` | use the powerup held | the same; hold to plant a Bone |
//! | F, `DISCARD` | throw away the powerup held | the same |
//! | E, `ACT`, on a mound | hold to dig | the same |
//! | E, `ACT`, on a mouth | a bark, since tunnels are Wolf-only | go through |
//! | E, `ACT`, on nothing | a bark | nothing |
//!
//! A mound the player can't dig yet, with a full slot, takes E and gives no
//! bark. A dug-up mound is no target, so E barks there.
//!
//! What each powerup does, here and in `snapshot.rs`. This is the table to
//! fill in (PLAN 14): each one already spends itself through `PowerupSlot`.
//!
//! | Powerup | Refused when | Effect |
//! | --- | --- | --- |
//! | Super appetite | | `Match::after_step`: the dial's speed |
//! | Bone | no tree or cairn near | `Match::plants`, a hold, then `Match::bone_spot`: a decoy mound |
//! | Howl | | `Match::use_powerup`: `Night::howl` |
//! | Stampede | | `Match::use_powerup`: `Flock::stampede` |
//! | Sniff Snoff | | `Match::draw_scents`, `Snapshot::scents`, a carcass's aura |
//! | Bell | no sheep under the crosshair, or it wears one | `Match::ring_bells` |
//! | Invisibility cloak | | `information::hidden`, in `Match::snapshot_for` |
//! | Sheep suit | | `BodyRow::suited`: the costume, seen by all |
//! | Carrot on a stick | | `Match::run_powerups`: `Flock::lure` and `Body::calm` |
//! | Frenzy | | `Match::frenzy_kills`, `Match::run_lunge` misses, `information::outlined` |
//! | Decoy sheep | this Sheepdog has one out | `Match::put_decoy`, `Match::bite_decoy`, `Snapshot::decoys` |
//! | Zoomies | | `Match::run_powerups`: `Body::sprint_scale` and stamina |
//! | Dognoculars | | `Facts::scoped`, `Match::spot_carcasses`, and the page's zoom |

pub mod aim;
mod bot;
mod snapshot;
#[cfg(test)]
mod tests;

pub use snapshot::{Action, BodyRow, CarcassRow, FallenRow, Me, Running, SheepRow, Snapshot, Target};

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use aim::Ray;

use crate::body::{Body, Input, Role, ACT, CROUCH, DISCARD, JUMP, PRIMARY, SECONDARY, SPRINT};
use crate::flock::Flock;
use crate::fx::{self, Fx};
use crate::ground::Ground;
use crate::lobby::{self, MatchStart};
use crate::map::{Map, Mover};
use crate::online::{Rollback, SEATS};
use crate::render::geom::v3;
use crate::rng::Rng;
use crate::rules::bark::BarkCooldown;
use crate::rules::bell::Bell;
use crate::rules::carcasses::{Carcass, CarcassTally};
use crate::rules::death::Life;
use crate::rules::decoy::{self, Decoy};
use crate::rules::eat::EatDial;
use crate::rules::fallen::Fallen;
use crate::rules::lunge::Lunge;
use crate::rules::mounds::{mounds_at_start, Digging, Dug, Mound, MoundClock, Planting};
use crate::rules::night::Night;
use crate::rules::outcome::{self, Goals, Outcome, Standing};
use crate::rules::powerups::{self, scent_offset, PowerupSlot};
use crate::rules::setup;
use crate::rules::timer::Timer;
use crate::rules::tune::{self, secs, Powerup};
use crate::rules::tunnels::TunnelCooldown;
use crate::tune::{MAX_PLAYERS, STAMINA_FULL, TICKS_PER_SECOND};
use crate::world::World;

/// The noise of a Bell set off: a cue of `data/sound.json`.
pub const BELL_NOISE: &str = "sheep_bell";

/// How often a dig under way makes its noise.
const DIG_SOUND_TICKS: u32 = 30;

/// Keeps a match's own dice apart from the map's on the same seed.
const DICE: u64 = 0x5eed_0f5e_ed0f_5eed;

/// The dice for Wolves' sheep ids, apart from the match's, so drawing them
/// changes no other roll.
const LABEL_DICE: u64 = 0x1abe_15de_c0de_5eed;

/// The page's movement keys, above `Input`'s buttons. The page sends bits and
/// `input_from_keys` turns them into one `Input`.
pub const KEY_FORWARD: u32 = 1 << 8;
pub const KEY_BACK: u32 = 1 << 9;
pub const KEY_LEFT: u32 = 1 << 10;
pub const KEY_RIGHT: u32 = 1 << 11;

/// Which key sets which bit, as `web/input.js` names them.
pub const KEYS: &[(&str, u32)] = &[
    ("KeyW", KEY_FORWARD),
    ("ArrowUp", KEY_FORWARD),
    ("KeyS", KEY_BACK),
    ("ArrowDown", KEY_BACK),
    ("KeyA", KEY_LEFT),
    ("ArrowLeft", KEY_LEFT),
    ("KeyD", KEY_RIGHT),
    ("ArrowRight", KEY_RIGHT),
    ("ShiftLeft", SPRINT as u32),
    ("ShiftRight", SPRINT as u32),
    ("AltLeft", CROUCH as u32),
    ("AltRight", CROUCH as u32),
    ("KeyC", CROUCH as u32),
    ("Space", JUMP as u32),
    ("KeyE", ACT as u32),
    ("KeyF", DISCARD as u32),
    ("Mouse0", PRIMARY as u32),
    ("Mouse2", SECONDARY as u32),
];

/// One tick's `Input` from the page's bits and where the player looks.
pub fn input_from_keys(bits: u32, aim: (u16, i16)) -> Input {
    let axis = |plus: u32, minus: u32| match (bits & plus != 0, bits & minus != 0) {
        (true, false) => 127,
        (false, true) => -127,
        _ => 0,
    };
    Input {
        strafe: axis(KEY_RIGHT, KEY_LEFT),
        forward: axis(KEY_FORWARD, KEY_BACK),
        yaw: aim.0,
        pitch: aim.1,
        buttons: (bits & 0xff) as u16,
    }
}

/// Everything the rules keep for one seat. `Match::players` runs parallel to
/// `World::bodies`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Player {
    pub seat: u8,
    pub role: Role,
    /// No person plays this seat, from the start or since a browser left.
    pub bot: bool,
    /// Where the body started, for `Match::moved`.
    start: (Fx, Fx),
    bark: BarkCooldown,
    eat: EatDial,
    lunge: Lunge,
    /// The way a Lunge goes, set at the click.
    lunge_way: (Fx, Fx),
    tunnel: TunnelCooldown,
    slot: PowerupSlot,
    digging: Digging,
    planting: Planting,
    life: Life,
    /// The bone from a decoy: this Sheepdog does nothing else until it's over.
    stun: Timer,
    /// A Wolf's scent, as an offset from it, while Sniff Snoff runs.
    scent: (Fx, Fx),
    /// What a bot keeps: all it knows lives inside the match.
    brain: bot::Brain,
}

/// A mound, and where it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MoundSpot {
    pub x: Fx,
    pub z: Fx,
    pub mound: Mound,
}

/// A carcass, and where it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CarcassSpot {
    pub x: Fx,
    pub z: Fx,
    pub carcass: Carcass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    map: Arc<Map>,
    pub world: World,
    pub players: Vec<Player>,
    pub goals: Goals,
    pub mounds: Vec<MoundSpot>,
    pub carcasses: Vec<CarcassSpot>,
    /// Wolves just killed, lying where they died.
    fallen: Vec<Fallen>,
    tally: CarcassTally,
    bells: Vec<Bell>,
    /// Decoy sheep on the ground, at most one a Sheepdog.
    decoys: Vec<Decoy>,
    /// What a Wolf's page calls each sheep id, real or decoy: see `decoy.rs`.
    /// Drawn from the seed, so it's no part of the checksum.
    wolf_labels: Arc<[u16]>,
    night: Night,
    mound_clock: MoundClock,
    /// The next of the map's spare mounds to come up.
    next_spare: usize,
    rng: Rng,
    eats: i32,
    elapsed: i32,
    outcome: Option<Outcome>,
    /// What just made a noise, for the mixer. Never read by a rule, so it's
    /// no part of the checksum.
    heard: Vec<Heard>,
}

/// One noise: a cue or group of `data/sound.json`, who made it and where.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Heard {
    pub tick: u32,
    pub name: &'static str,
    /// A seat, or a sheep for a Bell.
    pub who: u16,
    pub x: Fx,
    pub z: Fx,
}

/// How many noises a match keeps. A frame reads them all, and a frame runs at
/// most a few ticks.
const HEARD_KEPT: usize = 32;

/// The rules' team for a seat settled at match start. `Random` can't reach a
/// `MatchStart`.
fn team(role: lobby::Role) -> Role {
    match role {
        lobby::Role::Wolf => Role::Wolf,
        lobby::Role::Sheepdog | lobby::Role::Random => Role::Sheepdog,
    }
}

/// How far along the crosshair's line a thing on the ground at (x, z) lies,
/// if it's within `reach` of the body and on the line.
fn aimed(map: &Map, body: &Body, x: Fx, z: Fx, reach: Fx) -> Option<Fx> {
    let (dx, dz) = (x - body.x, z - body.z);
    if dx.abs() > reach || dz.abs() > reach || fx::length(dx, dz) > reach {
        return None;
    }
    let at = v3(x, map.height(x, z) + aim::TARGET_UP, z);
    Ray::of(body).hits(at, tune::AIM_WIDTH)
}

/// The FNV-1a hasher: the same bits on every machine, with sizes fixed so a
/// length reads the same on 32 and 64 bits.
struct Fnv(u64);

impl Hasher for Fnv {
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 ^= *b as u64;
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }

    fn write_usize(&mut self, n: usize) {
        self.write(&(n as u64).to_le_bytes());
    }

    fn write_isize(&mut self, n: isize) {
        self.write(&(n as i64).to_le_bytes());
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

impl Match {
    /// The match every browser builds from `start`, on the map of its seed and Map size.
    pub fn new(start: &MatchStart, map: Arc<Map>) -> Match {
        let mut rng = Rng::new(start.seed ^ DICE);
        let flock = Flock::on_map(&map, start.settings.sheep as usize, rng.next_u64());
        let mut players = Vec::with_capacity(MAX_PLAYERS);
        let mut bodies = Vec::with_capacity(MAX_PLAYERS);
        let (mut sheepdogs, mut wolves) = (0, 0);
        for seat in &start.seats {
            let role = team(seat.role);
            let (spots, used) = match role {
                Role::Sheepdog => (&map.sheepdog_starts, &mut sheepdogs),
                Role::Wolf => (&map.wolf_starts, &mut wolves),
            };
            let (x, z) = spots[*used % spots.len()];
            *used += 1;
            let (x, z) = map.push_out(x, z, role.breed().radius);
            let mut body = Body::new(role, x, z);
            body.settle(&*map);
            bodies.push(body);
            players.push(Player {
                seat: seat.seat as u8,
                role,
                bot: seat.person.is_empty(),
                start: (x, z),
                bark: BarkCooldown::default(),
                eat: EatDial::default(),
                lunge: Lunge::default(),
                lunge_way: (0, 0),
                tunnel: TunnelCooldown::default(),
                slot: PowerupSlot::default(),
                digging: Digging::default(),
                planting: Planting::default(),
                life: Life::default(),
                stun: Timer::default(),
                scent: (0, 0),
                brain: bot::Brain::default(),
            });
        }
        let powerups = start.settings.powerups;
        let seats = start.seats.len() as i32;
        let mounds = map
            .mounds
            .iter()
            .take(mounds_at_start(powerups, seats) as usize)
            .map(|m| MoundSpot {
                x: m.x,
                z: m.z,
                mound: Mound::drawn(&mut rng),
            })
            .collect();
        let ids = flock.sheep.len() + MAX_PLAYERS;
        let wolf_labels = decoy::labels(ids, &mut Rng::new(start.seed ^ LABEL_DICE)).into();
        Match {
            world: World::new(bodies, flock),
            players,
            goals: setup::goals(&start.settings, sheepdogs as i32),
            mounds,
            carcasses: Vec::new(),
            fallen: Vec::new(),
            tally: CarcassTally::default(),
            bells: Vec::new(),
            decoys: Vec::new(),
            wolf_labels,
            night: Night::default(),
            mound_clock: MoundClock::new(powerups, seats),
            next_spare: 0,
            rng,
            eats: 0,
            elapsed: 0,
            outcome: None,
            heard: Vec::new(),
            map,
        }
    }

    pub fn map(&self) -> &Map {
        &self.map
    }

    pub fn shared_map(&self) -> Arc<Map> {
        self.map.clone()
    }

    /// The noises of the last few ticks, oldest first.
    pub fn heard(&self) -> &[Heard] {
        &self.heard
    }

    fn noise(&mut self, name: &'static str, k: usize) {
        let body = &self.world.bodies[k];
        let (x, z) = (body.x, body.z);
        self.noise_at(name, k as u16, x, z);
    }

    fn noise_at(&mut self, name: &'static str, who: u16, x: Fx, z: Fx) {
        if self.heard.len() >= HEARD_KEPT {
            self.heard.remove(0);
        }
        self.heard.push(Heard {
            tick: self.world.tick,
            name,
            who,
            x,
            z,
        });
    }

    /// The look this seat starts with: toward the middle of the map.
    pub fn first_look(&self, seat: usize) -> (u16, i16) {
        let body = self.index_of(seat).map(|k| &self.world.bodies[k]);
        let (mx, mz) = (self.map.length() / 2, self.map.width() / 2);
        let (dx, dz) = body.map_or((0, 1), |b| (mx - b.x, mz - b.z));
        (aim::yaw_toward(dx, dz), aim::REST_PITCH)
    }

    /// How the match ended, once it has.
    pub fn outcome(&self) -> Option<Outcome> {
        self.outcome
    }

    pub fn standing(&self) -> Standing {
        Standing {
            eats: self.eats,
            in_fold: self.world.flock.folded as i32,
            carcasses_found: self.tally.carcasses_found(),
            wolves_alive: self.wolves_alive(),
            elapsed: self.elapsed,
        }
    }

    fn wolves_alive(&self) -> i32 {
        let alive = |p: &&Player| p.role == Role::Wolf && p.life.alive();
        self.players.iter().filter(alive).count() as i32
    }

    /// The player in this seat, by index into `players` and `World::bodies`.
    fn index_of(&self, seat: usize) -> Option<usize> {
        self.players.iter().position(|p| p.seat as usize == seat)
    }

    /// How far this seat's body stands from where it started, in `Fx` units
    /// across x and z. What the online check compares.
    pub fn moved(&self, seat: usize) -> Fx {
        let Some(k) = self.index_of(seat) else {
            return 0;
        };
        let (body, start) = (&self.world.bodies[k], self.players[k].start);
        (body.x - start.0).abs() + (body.z - start.1).abs()
    }

    /// One tick. `inputs` holds each seat's input, in seat order, and `None`
    /// for a seat with no person in it. Once the match is over, nothing moves.
    pub fn step(&mut self, inputs: &[Option<Input>; SEATS]) {
        if self.outcome.is_some() {
            return;
        }
        let mut wishes = [Input::default(); MAX_PLAYERS];
        for k in 0..self.players.len() {
            let input = match inputs[self.players[k].seat as usize] {
                Some(input) => input,
                None => {
                    self.players[k].bot = true;
                    self.bot_input(k)
                }
            };
            wishes[k] = self.act(k, input);
        }
        self.run_powerups();
        self.world.step(&*self.map, &wishes[..self.players.len()]);
        self.after_step();
        self.elapsed += 1;
        self.outcome = outcome::check(&self.goals, &self.standing());
    }

    /// What this player's buttons do this tick, and the `Input` left for the
    /// body.
    fn act(&mut self, k: usize, input: Input) -> Input {
        let now = self.world.tick;
        let look = Input {
            yaw: input.yaw,
            pitch: input.pitch,
            ..Input::default()
        };
        let mut body = self.world.bodies[k];
        if body.out {
            return look;
        }
        // the crosshair is where the player looks now
        body.yaw = input.yaw;
        body.pitch = input.pitch;
        let press = body.pressed(&input);
        let player = &mut self.players[k];
        if player.stun.running() {
            player.slot.release();
            player.digging.tick(None);
            return look;
        }
        if player.lunge.lunging() {
            player.digging.tick(None);
            self.world.bodies[k].locked_until = now + 1;
            return look;
        }
        if body.is_locked(now) {
            player.digging.tick(None);
            player.planting.tick(false);
            return input;
        }
        if press & DISCARD != 0 {
            player.slot.discard();
        }
        if input.buttons & SECONDARY == 0 {
            player.slot.release();
        } else if press & SECONDARY != 0 {
            self.use_powerup(k, &body);
        }
        if press & PRIMARY != 0 {
            match body.role {
                Role::Wolf => self.try_eat(k, &body),
                Role::Sheepdog => {
                    let player = &mut self.players[k];
                    if player.lunge.click() {
                        let (s, c) = (fx::sin(body.yaw as i32), fx::cos(body.yaw as i32));
                        player.lunge_way = (s, -c);
                    }
                }
            }
        }
        let planting = self.plants(k, &body, &input);
        if planting {
            self.world.bodies[k].locked_until = now + 1;
            if now.is_multiple_of(DIG_SOUND_TICKS) {
                self.noise("dig", k);
            }
        }
        if self.players[k].planting.tick(planting) {
            self.plant_bone(k, &body);
        }
        let held = input.buttons & ACT != 0 && !planting;
        self.use_e(k, &body, held, press & ACT != 0);
        input
    }

    /// Whether this player is planting a Bone this tick: on the ground with
    /// the button held, still, with a tree or cairn near. Like a dig it holds
    /// the body; unlike a dig, a move key gets out of it, since nothing under
    /// the crosshair can be lost.
    fn plants(&self, k: usize, body: &Body, input: &Input) -> bool {
        let still = input.strafe == 0 && input.forward == 0;
        self.players[k].slot.held() == Some(Powerup::Bone)
            && input.buttons & SECONDARY != 0
            && still
            && body.grounded
            && !self.world.bodies[k].is_locked(self.world.tick)
            && self.bone_spot(body).is_some()
    }

    fn plant_bone(&mut self, k: usize, body: &Body) {
        if let Some((x, z)) = self.bone_spot(body) {
            let mound = Mound::decoy();
            self.mounds.push(MoundSpot { x, z, mound });
            self.players[k].slot.activate();
            self.noise("powerup_activate", k);
        }
    }

    /// E: a mound under the crosshair, then a mouth, then a Sheepdog's bark.
    fn use_e(&mut self, k: usize, body: &Body, held: bool, press: bool) {
        let mut dig_at = None;
        if held {
            let free = self.players[k].slot.is_free();
            if let Some(i) = self.mound_under(body) {
                if self.mounds[i].mound.check(body.role, free).is_ok() {
                    dig_at = Some(i as u16);
                }
            } else if press {
                match (body.role, self.mouth_under(body)) {
                    (Role::Wolf, Some(mouth)) => self.enter_tunnel(k, mouth),
                    (Role::Wolf, None) => {}
                    (Role::Sheepdog, _) => {
                        if self.players[k].bark.bark() {
                            self.world.flock.bark(body.x, body.z);
                            self.noise("sheepdog_bark", k);
                        }
                    }
                }
            }
        }
        if dig_at.is_some() {
            self.world.bodies[k].locked_until = self.world.tick + 1;
        }
        if dig_at.is_some() && self.world.tick.is_multiple_of(DIG_SOUND_TICKS) {
            self.noise("dig", k);
        }
        if let Some(i) = self.players[k].digging.tick(dig_at) {
            self.finish_dig(k, i as usize);
        }
    }

    fn mound_under(&self, body: &Body) -> Option<usize> {
        let under = |(i, m): (usize, &MoundSpot)| {
            let open = m.mound.claimed_by().is_none();
            aimed(&self.map, body, m.x, m.z, tune::DIG_REACH)
                .filter(|_| open)
                .map(|d| (d, i))
        };
        self.mounds
            .iter()
            .enumerate()
            .filter_map(under)
            .min()
            .map(|(_, i)| i)
    }

    /// The nearest tunnel mouth under the crosshair. A tunnel's mouths are
    /// numbered 2t and 2t + 1, so the far one is this one's number with its
    /// last bit flipped.
    fn mouth_under(&self, body: &Body) -> Option<u16> {
        let mouths = self.map.mouths();
        let under =
            |(i, m): (usize, &(Fx, Fx))| aimed(&self.map, body, m.0, m.1, tune::TUNNEL_REACH).map(|d| (d, i));
        let near = mouths.iter().enumerate().filter_map(under).min();
        near.map(|(_, i)| i as u16)
    }

    fn enter_tunnel(&mut self, k: usize, mouth: u16) {
        let role = self.players[k].role;
        let Some(far) = self.players[k].tunnel.enter(role, mouth ^ 1) else {
            return;
        };
        let (x, z) = self.map.mouths()[far as usize];
        self.noise("tunnel_in", k);
        self.place(k, x, z);
        self.noise("tunnel_out", k);
    }

    /// Stands a body on this spot, at rest.
    fn place(&mut self, k: usize, x: Fx, z: Fx) {
        let (x, z) = self.map.push_out(x, z, self.players[k].role.breed().radius);
        let body = &mut self.world.bodies[k];
        (body.x, body.z) = (x, z);
        (body.vx, body.vz) = (0, 0);
        body.settle(&*self.map);
    }

    fn finish_dig(&mut self, k: usize, i: usize) {
        let player = &mut self.players[k];
        let free = player.slot.is_free();
        match self.mounds[i].mound.dig(k as u8, player.role, free) {
            Ok(Dug::Powerup(kind)) => {
                player.slot.take(kind);
                self.noise("powerup_found", k);
            }
            Ok(Dug::Decoy { stun_ticks }) => {
                player.stun.start(stun_ticks);
                player.slot.release();
                self.world.bodies[k].locked_until = self.world.tick + stun_ticks as u32;
            }
            Err(_) => {}
        }
    }

    /// The nearest sheep under the crosshair that can be eaten or belled.
    fn sheep_under(&self, body: &Body, reach: Fx) -> Option<u16> {
        let under = |(i, s): (usize, &crate::flock::Sheep)| {
            let open = !s.eaten && !s.folded;
            if !open {
                return None;
            }
            aimed(&self.map, body, s.x, s.z, reach).map(|d| (d, i))
        };
        let sheep = self.world.flock.sheep.iter().enumerate();
        sheep.filter_map(under).min().map(|(_, i)| i as u16)
    }

    /// What a Wolf's bite would land on: the nearest sheep or Decoy sheep
    /// under the crosshair, by id (`decoy::id` for a decoy).
    fn prey_under(&self, body: &Body) -> Option<u16> {
        let n = self.world.flock.sheep.len();
        let sheep = self.sheep_under(body, tune::EAT_REACH).map(|i| {
            let s = &self.world.flock.sheep[i as usize];
            (aimed(&self.map, body, s.x, s.z, tune::EAT_REACH), i)
        });
        let decoys = self.decoys.iter().map(|d| {
            let id = decoy::id(n, d.owner as usize);
            (aimed(&self.map, body, d.x, d.z, tune::EAT_REACH), id)
        });
        let near = sheep
            .into_iter()
            .chain(decoys)
            .filter_map(|(d, i)| d.map(|d| (d, i)));
        near.min().map(|(_, i)| i)
    }

    fn try_eat(&mut self, k: usize, body: &Body) {
        if !self.players[k].eat.ready() {
            return;
        }
        let Some(sheep) = self.prey_under(body) else {
            return;
        };
        let now = self.world.tick;
        let player = &mut self.players[k];
        // a bite at a decoy spends the dial and breaks a cloak like any bite
        player.eat.eat(now as i32);
        player.slot.ate();
        self.world.bodies[k].locked_until = player.eat.lock_ends() as u32;
        if let Some(owner) = decoy::owner_of(sheep, self.world.flock.sheep.len()) {
            self.bite_decoy(k, owner);
            return;
        }
        let flock = &mut self.world.flock;
        flock.eat(sheep as usize);
        let eaten = &flock.sheep[sheep as usize];
        self.carcasses.push(CarcassSpot {
            x: eaten.x,
            z: eaten.z,
            carcass: Carcass::default(),
        });
        self.eats += 1;
        self.noise("wolf_eat", k);
    }

    /// A right-click press with something held. See the table at the top.
    fn use_powerup(&mut self, k: usize, body: &Body) {
        let Some(kind) = self.players[k].slot.held() else {
            return;
        };
        match kind {
            // a hold, not a press: `Match::plants`
            Powerup::Bone => return,
            Powerup::Bell => {
                let Some(sheep) = self.bell_target(body) else {
                    return;
                };
                self.bells.push(Bell::hang(sheep));
            }
            Powerup::Howl => {
                self.night.howl();
                self.noise("wolf_howl", k);
            }
            Powerup::Stampede => {
                self.world.flock.stampede(body.x, body.z);
                self.noise("stampede", k);
            }
            Powerup::SniffSnoff => self.draw_scents(),
            Powerup::DecoySheep => {
                if self.decoys.iter().any(|d| d.owner as usize == k) {
                    return;
                }
                self.put_decoy(k, body);
            }
            _ => {}
        }
        self.players[k].slot.activate();
        self.noise("powerup_activate", k);
    }

    /// The id behind a Wolf's label: for a Wolf bot to steer by, never to
    /// decide by.
    fn unlabel(&self, label: u16) -> u16 {
        let at = self.wolf_labels.iter().position(|l| *l == label);
        at.expect("every label names an id") as u16
    }

    /// Where a sheep or Decoy sheep is, by id, while it's still there.
    fn sheep_at(&self, id: u16) -> Option<(Fx, Fx)> {
        let flock = &self.world.flock.sheep;
        match decoy::owner_of(id, flock.len()) {
            None => {
                let s = &flock[id as usize];
                (!s.eaten).then_some((s.x, s.z))
            }
            Some(owner) => {
                let d = self.decoys.iter().find(|d| d.owner as usize == owner);
                d.map(|d| (d.x, d.z))
            }
        }
    }

    /// A Decoy sheep goes down just ahead of this Sheepdog, looking like the
    /// real sheep nearest it.
    fn put_decoy(&mut self, k: usize, body: &Body) {
        let (x, z) = decoy::spot(body.x, body.z, body.yaw);
        let like = self.world.flock.sheep.iter().filter(|s| !s.eaten);
        let like = like.min_by_key(|s| fx::length(s.x - x, s.z - z));
        let (size, fleece, kind) = like.map_or((fx::ONE, 0, crate::flock::Kind::Ewe), |s| {
            (s.size, s.fleece, s.kind)
        });
        let radius = fx::mul(crate::tune::SHEEP_RADIUS, size);
        let (x, z) = self.map.push_out(x, z, radius);
        let (s, c) = (fx::sin(body.yaw as i32), fx::cos(body.yaw as i32));
        self.decoys.push(Decoy {
            owner: k as u8,
            x,
            z,
            face_x: s,
            face_z: -c,
            size,
            fleece,
            kind,
        });
    }

    /// A Wolf bit this Sheepdog's Decoy sheep: no eat, the decoy is gone, and
    /// the Wolf is stunned, its outline showing to Sheepdogs till it's free.
    fn bite_decoy(&mut self, k: usize, owner: usize) {
        self.decoys.retain(|d| d.owner as usize != owner);
        let ticks = secs(tune::DECOY_STUN_SECONDS);
        let player = &mut self.players[k];
        player.stun.start(ticks);
        player.slot.release();
        self.world.bodies[k].locked_until = self.world.tick + ticks as u32;
        self.noise("wolf_eat", k);
    }

    /// The sheep a Bell would go on: under the crosshair, and wearing none.
    fn bell_target(&self, body: &Body) -> Option<u16> {
        let sheep = self.sheep_under(body, tune::BELL_HANG_REACH);
        sheep.filter(|s| !self.bells.iter().any(|b| b.sheep() == *s))
    }

    /// Where a Bone plants its decoy: at the base of the nearest tree or
    /// cairn in reach, on this Wolf's side of it.
    fn bone_spot(&self, body: &Body) -> Option<(Fx, Fx)> {
        let trees = self.map.trees.iter().map(|t| (t.x, t.z, t.trunk));
        let cairns = self.map.cairns.iter().map(|c| (c.x, c.z, c.tiers[0].radius));
        let mut best = None;
        for (x, z, base) in trees.chain(cairns) {
            let (dx, dz) = (body.x - x, body.z - z);
            let reach = tune::BONE_PLANT_REACH + base;
            if dx.abs() > reach || dz.abs() > reach {
                continue;
            }
            let d = fx::length(dx, dz);
            if d <= reach && best.is_none_or(|(near, _)| d - base < near) {
                let out = base + tune::BONE_BASE_GAP;
                let spot = if d == 0 {
                    (x + out, z)
                } else {
                    (x + fx::mul(dx, fx::div(out, d)), z + fx::mul(dz, fx::div(out, d)))
                };
                best = Some((d - base, spot));
            }
        }
        best.map(|(_, spot)| spot)
    }

    /// Fresh scents for every Wolf, which come out in player order.
    fn draw_scents(&mut self) {
        for player in self.players.iter_mut().filter(|p| p.role == Role::Wolf) {
            player.scent = scent_offset(&mut self.rng);
        }
    }

    /// What a running powerup does to the world each tick, before it steps.
    fn run_powerups(&mut self) {
        for (player, body) in self.players.iter().zip(&mut self.world.bodies) {
            let carrot = player.slot.running(Powerup::CarrotOnAStick) && !body.out;
            let zoomies = player.slot.running(Powerup::Zoomies);
            body.calm = carrot;
            body.sprint_scale = if zoomies {
                powerups::zoomies_sprint(fx::ONE)
            } else {
                fx::ONE
            };
            if zoomies {
                body.stamina = STAMINA_FULL;
            }
            if carrot {
                self.world.flock.lure(body.x, body.z);
            }
        }
    }

    /// Whether this seat looks through Dognoculars now.
    pub fn scoped(&self, seat: usize) -> bool {
        let player = self.index_of(seat).map(|k| &self.players[k]);
        player.is_some_and(|p| p.slot.running(Powerup::Dognoculars))
    }

    /// Dognoculars up for this seat, as a test wants them.
    #[cfg(test)]
    pub fn scope_up(&mut self, seat: usize) {
        let k = self.index_of(seat).expect("a seat with a player");
        self.players[k].slot.take(Powerup::Dognoculars);
        self.players[k].slot.activate();
    }

    /// Dognoculars put away, as a test wants them.
    #[cfg(test)]
    pub fn scope_down(&mut self, seat: usize) {
        let k = self.index_of(seat).expect("a seat with a player");
        self.players[k].slot.release();
    }

    /// The rules that run after the bodies have moved.
    fn after_step(&mut self) {
        self.fallen.retain_mut(|f| !f.tick());
        for k in 0..self.players.len() {
            self.run_lunge(k);
            let player = &mut self.players[k];
            player.bark.tick();
            player.tunnel.tick();
            player.slot.tick();
            player.stun.tick();
            if player.life.alive() {
                player.eat.tick(player.slot.running(Powerup::SuperAppetite));
            }
            if player.life.tick() {
                match player.role {
                    Role::Wolf => self.wolf_returns(k),
                    Role::Sheepdog => self.sheepdog_returns(k),
                }
            }
        }
        self.frenzy_kills();
        self.night.tick();
        self.ring_bells();
        self.spot_carcasses();
        if self.mound_clock.tick() {
            self.new_mound();
        }
        let sniffing = self.players.iter().any(|p| p.slot.running(Powerup::SniffSnoff));
        if sniffing && (self.elapsed + 1) % secs(tune::SCENT_REDRAW_SECONDS) == 0 {
            self.draw_scents();
        }
    }

    /// One tick of a Sheepdog's Lunge: it carries the body along its line,
    /// and a Wolf within touching distance at any moment dies.
    fn run_lunge(&mut self, k: usize) {
        if !self.players[k].lunge.lunging() {
            self.players[k].lunge.tick();
            return;
        }
        let (way_x, way_z) = self.players[k].lunge_way;
        let stride = tune::LUNGE_STRIDE;
        let body = &mut self.world.bodies[k];
        let wanted = (body.x + fx::mul(way_x, stride), body.z + fx::mul(way_z, stride));
        // feet on the ground: a Lunge from a jump clears no wall or fence
        let feet = body.y.min(self.map.height(body.x, body.z));
        let mover = Mover::Sheepdog { feet };
        let radius = body.role.breed().radius;
        (body.x, body.z) = self.map.push_out_as(wanted.0, wanted.1, radius, mover);
        if body.grounded {
            body.y = self.map.height(body.x, body.z);
        }
        let (x, z) = (body.x, body.z);
        let touching = |j: &usize| {
            let (p, b) = (&self.players[*j], &self.world.bodies[*j]);
            let (dx, dz) = (b.x - x, b.z - z);
            let close = dx.abs() <= tune::LUNGE_CONTACT && dz.abs() <= tune::LUNGE_CONTACT;
            // a Wolf in a Frenzy can't be killed: the Lunge goes through it
            let frenzied = p.slot.running(Powerup::Frenzy);
            p.role == Role::Wolf
                && p.life.alive()
                && !frenzied
                && close
                && fx::length(dx, dz) <= tune::LUNGE_CONTACT
        };
        match (0..self.players.len()).find(touching) {
            Some(wolf) => {
                self.players[k].lunge.hit();
                self.noise("lunge_hit", k);
                self.kill(wolf, k);
            }
            None => {
                if self.players[k].lunge.tick() {
                    self.world.flock.scatter(x, z);
                }
            }
        }
    }

    /// Every Wolf in a Frenzy kills each Sheepdog its body touches.
    fn frenzy_kills(&mut self) {
        for w in 0..self.players.len() {
            let (p, b) = (&self.players[w], &self.world.bodies[w]);
            if p.role != Role::Wolf || b.out || !p.slot.running(Powerup::Frenzy) {
                continue;
            }
            let (x, z) = (b.x, b.z);
            let touching = |j: &usize| {
                let (p, b) = (&self.players[*j], &self.world.bodies[*j]);
                let (dx, dz) = (b.x - x, b.z - z);
                let close = dx.abs() <= tune::FRENZY_CONTACT && dz.abs() <= tune::FRENZY_CONTACT;
                let dog = p.role == Role::Sheepdog && p.life.alive() && !b.out;
                dog && close && fx::length(dx, dz) <= tune::FRENZY_CONTACT
            };
            let hit: Vec<usize> = (0..self.players.len()).filter(touching).collect();
            for dog in hit {
                self.noise("lunge_hit", w);
                self.kill(dog, w);
            }
        }
    }

    /// A Wolf dies to a Lunge, or a Sheepdog to a Frenzy, and what it held
    /// goes with it. Its body lies where it fell.
    fn kill(&mut self, k: usize, killer: usize) {
        let (body, seat) = (&self.world.bodies[k], self.players[killer].seat);
        let role = self.players[k].role;
        self.fallen.push(match role {
            Role::Wolf => Fallen::new(body.x, body.z, body.yaw, seat),
            Role::Sheepdog => Fallen::sheepdog(body.x, body.z, body.yaw, seat),
        });
        let player = &mut self.players[k];
        // Sudden death is about Wolves: a Sheepdog always comes back
        player.life.kill(self.goals.sudden_death && role == Role::Wolf);
        player.slot = PowerupSlot::default();
        player.digging = Digging::default();
        player.planting = Planting::default();
        player.lunge = Lunge::default();
        let body = &mut self.world.bodies[k];
        body.out = true;
        body.crouched = false;
    }

    /// A Wolf back from the dead comes out of a random mouth away from every
    /// Sheepdog, or the mouth farthest from them when none is.
    fn wolf_returns(&mut self, k: usize) {
        let mouths = self.map.mouths().to_vec();
        self.comes_back(k, &mouths, Role::Sheepdog);
    }

    /// A Sheepdog back from a Frenzy comes back at a Sheepdog start away from
    /// every Wolf, or the one farthest from them when none is.
    fn sheepdog_returns(&mut self, k: usize) {
        let starts = self.map.sheepdog_starts.clone();
        self.comes_back(k, &starts, Role::Wolf);
    }

    /// Back from the dead at a random one of these spots clear of the other
    /// team, or the one farthest from it when none is.
    fn comes_back(&mut self, k: usize, spots: &[(Fx, Fx)], foe: Role) {
        let foes: Vec<(Fx, Fx)> = self
            .players
            .iter()
            .zip(&self.world.bodies)
            .filter(|(p, _)| p.role == foe)
            .map(|(_, b)| (b.x, b.z))
            .collect();
        let nearest_foe = |m: &(Fx, Fx)| {
            let far = foes.iter().map(|d| fx::length(d.0 - m.0, d.1 - m.1));
            far.min().unwrap_or(Fx::MAX)
        };
        let clear: Vec<(Fx, Fx)> = spots
            .iter()
            .copied()
            .filter(|m| nearest_foe(m) >= tune::RETURN_CLEAR)
            .collect();
        let (x, z) = if clear.is_empty() {
            spots
                .iter()
                .copied()
                .max_by_key(nearest_foe)
                .expect("a map has spots to come back at")
        } else {
            clear[self.rng.below(clear.len() as u32) as usize]
        };
        self.place(k, x, z);
        let body = &mut self.world.bodies[k];
        body.out = false;
        body.held = 0;
    }

    /// Every Bell hears how near the nearest Wolf is. A silent Bell whose
    /// sheep is eaten or folded comes off; a ringing one finishes its ring.
    fn ring_bells(&mut self) {
        let mut set_off = Vec::new();
        let world = &self.world;
        let wolves = || {
            let alive = self.players.iter().zip(&world.bodies);
            alive
                .filter(|(p, b)| p.role == Role::Wolf && !b.out)
                .map(|(_, b)| b)
        };
        for bell in &mut self.bells {
            let sheep = &world.flock.sheep[bell.sheep() as usize];
            let far = wolves().map(|b| fx::length(b.x - sheep.x, b.z - sheep.z));
            let silent = !bell.ringing();
            bell.tick(far.min());
            if silent && bell.ringing() {
                set_off.push((bell.sheep(), sheep.x, sheep.z));
            }
        }
        self.bells.retain(|bell| {
            let sheep = &world.flock.sheep[bell.sheep() as usize];
            let silent_and_gone = !bell.ringing() && (sheep.eaten || sheep.folded);
            !bell.used_up() && !silent_and_gone
        });
        for (sheep, x, z) in set_off {
            self.noise_at(BELL_NOISE, sheep, x, z);
        }
    }

    /// A Sheepdog finds a carcass by putting the crosshair on it with a clear
    /// line of sight.
    fn spot_carcasses(&mut self) {
        for i in 0..self.carcasses.len() {
            let spot = self.carcasses[i];
            if spot.carcass.found() {
                continue;
            }
            let sees = |(p, b): (&Player, &Body)| {
                let reach = if p.slot.running(Powerup::Dognoculars) {
                    tune::DOGNOCULARS_CARCASS_REACH
                } else {
                    tune::CARCASS_SIGHT_REACH
                };
                let on_it = p.role == Role::Sheepdog && aimed(&self.map, b, spot.x, spot.z, reach).is_some();
                on_it && self.clear_line((b.x, b.z), (spot.x, spot.z))
            };
            let sighted = self.players.iter().zip(&self.world.bodies).any(sees);
            self.tally.spot(&mut self.carcasses[i].carcass, sighted);
        }
    }

    /// No wall, trunk or cairn stands between two points, tested at each step
    /// along the line.
    fn clear_line(&self, from: (Fx, Fx), to: (Fx, Fx)) -> bool {
        let (dx, dz) = (to.0 - from.0, to.1 - from.1);
        let length = fx::length(dx, dz);
        let steps = length / tune::SIGHT_STEP;
        let thin = fx::ONE / 10;
        (1..steps).all(|n| {
            let share = fx::ratio(n, steps);
            let (x, z) = (from.0 + fx::mul(dx, share), from.1 + fx::mul(dz, share));
            self.map.push_out_as(x, z, thin, Mover::Sight) == (x, z)
        })
    }

    /// The next spare mound comes up, until the map runs out of them.
    fn new_mound(&mut self) {
        if let Some(m) = self.map.spare_mounds.get(self.next_spare) {
            self.next_spare += 1;
            self.mounds.push(MoundSpot {
                x: m.x,
                z: m.z,
                mound: Mound::drawn(&mut self.rng),
            });
        }
    }

    /// The seconds left on the match clock, rounded up.
    pub fn seconds_left(&self) -> i32 {
        let ticks = TICKS_PER_SECOND;
        ((self.goals.match_ticks - self.elapsed).max(0) + ticks - 1) / ticks
    }
}

impl Rollback for Match {
    const NOBODY: u64 = u64::MAX;

    fn step(&mut self, keys: [u64; SEATS]) {
        let inputs = keys.map(|k| (k != Self::NOBODY).then(|| Input::unpack(k)));
        Match::step(self, &inputs);
    }

    /// FNV-1a over everything that changes in a match, in a fixed order.
    fn sum(&self) -> u64 {
        let mut h = Fnv(0xcbf2_9ce4_8422_2325);
        self.world.tick.hash(&mut h);
        self.world.bodies.hash(&mut h);
        self.world.flock.hash_state(&mut h);
        self.players.hash(&mut h);
        self.mounds.hash(&mut h);
        self.carcasses.hash(&mut h);
        self.fallen.hash(&mut h);
        self.tally.hash(&mut h);
        self.bells.hash(&mut h);
        self.decoys.hash(&mut h);
        self.night.hash(&mut h);
        self.mound_clock.hash(&mut h);
        self.next_spare.hash(&mut h);
        self.rng.hash(&mut h);
        self.eats.hash(&mut h);
        self.elapsed.hash(&mut h);
        self.outcome.map(|o| o as u8).hash(&mut h);
        h.finish()
    }
}
