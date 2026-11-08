//! What the page draws for one viewer. `information::view` says what's
//! one-sided, and this adds where things are, filtered by what that view
//! allows, so a viewer's `Snapshot` holds nothing that viewer isn't to know.
//!
//! The list of sheep is one of those things. Eaten sheep leave it, so a whole
//! list would count the sheep alive, and FL-7 keeps that from Sheepdogs. It
//! holds the sheep in sight alone, and the same goes for carcasses.
//!
//! Positions and fractions are raw `Fx`, 65536 to one.

use serde::Serialize;

use super::{aim, Match};
use crate::fx::{self, Fx};
use crate::map::Map;
use crate::rules::decoy;
use crate::rules::information::{self, in_sight, sheep_label, Facts, View};
use crate::rules::outcome::Outcome;
use crate::rules::powerups;
use crate::rules::tune::{self, Powerup};
use crate::rules::Role;
use crate::tune::{STAMINA_FULL, TICKS_PER_SECOND};

/// What E would act on, for the viewer to highlight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    Mound(u16),
    Mouth(u16),
}

/// What a body is busy with, for its clip. Anyone in sight can see these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Moving,
    /// Held in place, eating.
    Snapping,
    /// A Sheepdog's Lunge: carried along its line, flat out.
    Lunging,
    Digging,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BodyRow {
    pub seat: u8,
    pub role: Role,
    pub x: Fx,
    pub y: Fx,
    pub z: Fx,
    pub yaw: u16,
    pub pitch: i16,
    pub crouched: bool,
    pub sprinting: bool,
    pub grounded: bool,
    /// Meters a second across the ground.
    pub speed: Fx,
    pub action: Action,
    /// Spent, and breathing hard.
    pub winded: bool,
    /// Whether this viewer sees this body's outline, through walls too.
    pub outline: bool,
    /// In the Sheep suit: seen by all.
    pub suited: bool,
    /// Under the Invisibility cloak: only its own team gets this row at all.
    pub cloaked: bool,
    /// Holding out a Carrot on a stick.
    pub carrot: bool,
    /// In a Frenzy: outlined to everyone.
    pub frenzy: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SheepRow {
    pub id: u16,
    pub x: Fx,
    pub z: Fx,
    pub face_x: Fx,
    pub face_z: Fx,
    pub size: Fx,
    pub fleece: u8,
    pub kind: crate::flock::Kind,
    /// Meters a second.
    pub speed: Fx,
    pub hop: Fx,
    pub grazing: bool,
    pub folded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CarcassRow {
    pub x: Fx,
    pub z: Fx,
    /// Sniff Snoff's scent rises from it.
    pub aura: bool,
}

/// A timed powerup of the viewer's, running: its icon, name and what's left.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Running {
    pub icon: &'static str,
    pub name: &'static str,
    pub left: Fx,
}

/// A Wolf just killed. Always a Wolf to look at, whatever it wore: being caught
/// is the reveal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FallenRow {
    /// A Wolf to a Lunge, or a Sheepdog to a Frenzy.
    pub role: Role,
    pub x: Fx,
    pub z: Fx,
    pub yaw: u16,
    /// Ticks since the kill.
    pub age: i32,
}

/// The viewer's own bars and dials. A team's dial is left out for the other.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Me {
    pub seat: u8,
    pub role: Role,
    pub alive: bool,
    /// Whole seconds until a killed Wolf returns.
    pub back_in: Option<i32>,
    pub stunned: bool,
    /// Sprint left: 0 empty, 1 full.
    pub stamina: Fx,
    /// A dig or a Bone's planting under way: 0 neither, 1 done.
    pub dig: Fx,
    /// The icon id and name of the powerup held, from the table.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub held: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub held_name: Option<&'static str>,
    pub held_left: Fx,
    /// The timers running after their powerup left the slot.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub running: Vec<Running>,
    /// Looking through Dognoculars.
    pub scoped: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eat_dial: Option<Fx>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tunnel_dial: Option<Fx>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lunge_dial: Option<Fx>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    /// The sheep a left click would eat: a Wolf's red outline. A Sheepdog
    /// gets no such thing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prey: Option<u16>,
    /// The sheep a right click would hang the Bell on, for a Sheepdog holding
    /// one: its outline.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub belling: Option<u16>,
    /// This Sheepdog just killed a Wolf: its banner shows.
    pub killed: bool,
    /// The flash on the killer's screen: 0 none, 1 at the kill.
    pub kill_flash: Fx,
    /// This Wolf is pressing against the Fold, so the Fold's message shows.
    pub at_fold: bool,
    /// Seconds until the dial is back: rounded up, 0 when ready.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eat_seconds: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    pub tick: u32,
    pub seconds_left: i32,
    pub fold_target: i32,
    pub eat_goal: i32,
    /// The Howl's night: 0 day, 1 night.
    pub darkness: Fx,
    /// Everything one-sided: counts, Bells, how each mound looks, how far
    /// this viewer sees.
    pub view: View,
    /// Where each mound is, in the order of `view.mounds`.
    pub mound_spots: Vec<[Fx; 2]>,
    pub bodies: Vec<BodyRow>,
    /// The seats in `bodies` this viewer sees a nameplate over
    /// (`information::sees_plate`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub plates: Vec<u8>,
    pub sheep: Vec<SheepRow>,
    pub carcasses: Vec<CarcassRow>,
    /// Wolves just killed.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fallen: Vec<FallenRow>,
    /// Where a Wolf's scent rises, near it and never on it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scents: Vec<[Fx; 2]>,
    /// Which rows of `sheep` are Decoy sheep, by id: Sheepdogs alone get
    /// these, and outline them.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub decoys: Vec<u16>,
    pub me: Me,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<Outcome>,
}

impl Match {
    /// What this seat's page draws, or `None` for a seat no one holds.
    pub fn snapshot_for(&self, seat: usize) -> Option<Snapshot> {
        let k = self.index_of(seat)?;
        let (player, me) = (&self.players[k], &self.world.bodies[k]);
        let spots: Vec<_> = self.mounds.iter().map(|m| m.mound).collect();
        let standing = self.standing();
        let view = information::view(
            player.role,
            &Facts {
                standing: &standing,
                bells: &self.bells,
                fallen: &self.fallen,
                mounds: &spots,
                darkness: self.night.darkness(),
                scoped: player.slot.running(Powerup::Dognoculars),
            },
        );
        let range = view.sight_range;
        let from = (me.x, me.z);
        let sniffing = player.slot.running(Powerup::SniffSnoff);
        let aura = information::smells(player.role, sniffing);
        let body_row = |(p, b): (&super::Player, &crate::body::Body)| {
            let frenzy = p.slot.running(Powerup::Frenzy);
            let outline = information::outlined(player.role, p.role, p.stun.running(), frenzy);
            let cloaked = p.slot.running(Powerup::InvisibilityCloak);
            let cloak_holds = information::cloak_holds(cloaked, frenzy);
            let hidden = information::hidden(player.role, p.role, cloak_holds);
            let shown = !b.out && !hidden && (outline || in_sight(range, from, (b.x, b.z)));
            shown.then_some(BodyRow {
                seat: p.seat,
                role: p.role,
                x: b.x,
                y: b.y,
                z: b.z,
                yaw: if p.lunge.lunging() {
                    aim::yaw_toward(p.lunge_way.0, p.lunge_way.1)
                } else {
                    b.yaw
                },
                pitch: b.pitch,
                crouched: b.crouched,
                sprinting: b.sprinting,
                grounded: b.grounded,
                speed: if p.lunge.lunging() {
                    tune::LUNGE_STRIDE * TICKS_PER_SECOND
                } else {
                    fx::length(b.vx, b.vz)
                },
                action: if p.digging.fraction() > 0 || p.planting.fraction() > 0 {
                    Action::Digging
                } else if p.lunge.lunging() {
                    Action::Lunging
                } else if b.is_locked(self.world.tick) {
                    Action::Snapping
                } else {
                    Action::Moving
                },
                winded: b.winded,
                outline,
                suited: p.slot.running(Powerup::SheepSuit),
                cloaked,
                carrot: p.slot.running(Powerup::CarrotOnAStick),
                frenzy,
            })
        };
        let labels = &self.wolf_labels;
        let label = |id: u16| sheep_label(player.role, id, labels);
        let sheep = self.world.flock.sheep.iter().enumerate();
        let sheep_row = |(i, s): (usize, &crate::flock::Sheep)| {
            let shown = !s.eaten && in_sight(range, from, (s.x, s.z));
            shown.then_some(SheepRow {
                id: label(i as u16),
                x: s.x,
                z: s.z,
                face_x: s.face_x,
                face_z: s.face_z,
                size: s.size,
                fleece: s.fleece,
                kind: s.kind,
                speed: fx::length(s.vx, s.vz),
                hop: s.hop_height(),
                grazing: s.grazing(),
                folded: s.folded,
            })
        };
        // a Decoy sheep is a grazing sheep standing still, row for row
        let flock = self.world.flock.sheep.len();
        let decoy_row = |d: &decoy::Decoy| {
            in_sight(range, from, (d.x, d.z)).then_some(SheepRow {
                id: label(decoy::id(flock, d.owner as usize)),
                x: d.x,
                z: d.z,
                face_x: d.face_x,
                face_z: d.face_z,
                size: d.size,
                fleece: d.fleece,
                kind: d.kind,
                speed: 0,
                hop: 0,
                grazing: true,
                folded: false,
            })
        };
        let mut sheep: Vec<SheepRow> = sheep.filter_map(sheep_row).collect();
        sheep.extend(self.decoys.iter().filter_map(decoy_row));
        // in id order, so where a row sits says nothing of what it is
        sheep.sort_by_key(|r| r.id);
        let decoys = self
            .decoys
            .iter()
            .filter(|_| information::sees_decoys(player.role));
        let decoys = decoys.map(|d| decoy::id(flock, d.owner as usize)).collect();
        let carcass_row = |c: &super::CarcassSpot| {
            in_sight(range, from, (c.x, c.z)).then_some(CarcassRow { x: c.x, z: c.z, aura })
        };
        // a body by the same rule as the living: its team sees it anywhere
        let fallen_row = |f: &crate::rules::fallen::Fallen| {
            let outlined = information::sees_outline(player.role, f.role);
            let shown = outlined || in_sight(range, from, (f.x, f.z));
            shown.then_some(FallenRow {
                role: f.role,
                x: f.x,
                z: f.z,
                yaw: f.yaw,
                age: f.age(),
            })
        };
        let bodies: Vec<BodyRow> = self
            .players
            .iter()
            .zip(&self.world.bodies)
            .filter_map(body_row)
            .collect();
        let plates = bodies
            .iter()
            .filter(|b| information::sees_plate(player.role, b.role, b.seat == player.seat))
            .map(|b| b.seat)
            .collect();
        let wolves = self.players.iter().zip(&self.world.bodies);
        let scents = wolves
            .filter(|(p, b)| aura && p.role == Role::Wolf && !b.out)
            .map(|(p, b)| [b.x + p.scent.0, b.z + p.scent.1])
            .collect();
        Some(Snapshot {
            tick: self.world.tick,
            seconds_left: self.seconds_left(),
            fold_target: self.goals.fold_target,
            eat_goal: self.goals.eats_required,
            darkness: self.night.darkness(),
            mound_spots: self.mounds.iter().map(|m| [m.x, m.z]).collect(),
            bodies,
            plates,
            sheep,
            carcasses: self.carcasses.iter().filter_map(carcass_row).collect(),
            fallen: self.fallen.iter().filter_map(fallen_row).collect(),
            scents,
            decoys,
            me: self.me(k),
            outcome: self.outcome,
            view,
        })
    }

    fn me(&self, k: usize) -> Me {
        let (player, body) = (&self.players[k], &self.world.bodies[k]);
        let wolf = player.role == Role::Wolf;
        let held = player.slot.held().map(powerups::row);
        // the banner and flash are a Sheepdog's, for a Wolf killed
        let mine = || {
            let wolves = self.fallen.iter().filter(|f| f.role == Role::Wolf);
            wolves.filter(|f| f.killer == player.seat)
        };
        Me {
            seat: player.seat,
            role: player.role,
            alive: player.life.alive(),
            back_in: player.life.back_in(),
            stunned: player.stun.running(),
            stamina: fx::ratio(body.stamina / (STAMINA_FULL / 3600), 3600),
            dig: player.digging.fraction().max(player.planting.fraction()),
            held: held.map(|r| r.icon),
            held_name: held.map(|r| r.name),
            held_left: player.slot.held_fraction(),
            running: player
                .slot
                .timers()
                .map(|r| Running {
                    icon: r.icon,
                    name: r.name,
                    left: player.slot.running_fraction(r.kind),
                })
                .collect(),
            scoped: player.slot.running(Powerup::Dognoculars),
            eat_dial: wolf.then(|| player.eat.fraction()),
            tunnel_dial: wolf.then(|| player.tunnel.fraction()),
            lunge_dial: (!wolf).then(|| player.lunge.wait_fraction()),
            target: self.target(k),
            prey: (wolf && !body.out)
                .then(|| self.prey_under(body))
                .flatten()
                .map(|id| information::sheep_label(player.role, id, &self.wolf_labels)),
            belling: (player.slot.held() == Some(Powerup::Bell) && !body.out)
                .then(|| self.bell_target(body))
                .flatten(),
            killed: mine().any(|f| f.news()),
            kill_flash: mine().map(|f| f.flash()).max().unwrap_or(0),
            at_fold: wolf && pressing_fold(self.map(), body),
            eat_seconds: wolf.then(|| player.eat.seconds_left()),
        }
    }

    /// What's under the crosshair for this player, in the order E looks.
    fn target(&self, k: usize) -> Option<Target> {
        let body = &self.world.bodies[k];
        if body.out {
            return None;
        }
        if let Some(i) = self.mound_under(body) {
            return Some(Target::Mound(i as u16));
        }
        if body.role == Role::Sheepdog {
            return None;
        }
        self.mouth_under(body).map(Target::Mouth)
    }
}

/// A step ahead of this Wolf, would the Fold stop it where a Sheepdog would
/// pass?
fn pressing_fold(map: &Map, body: &crate::body::Body) -> bool {
    let (s, c) = (fx::sin(body.yaw as i32), fx::cos(body.yaw as i32));
    let (x, z) = (body.x + s, body.z - c);
    let radius = body.role.breed().radius;
    map.fold_stops_wolf(x, z, radius, body.y)
}
