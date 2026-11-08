//! A match on the real hillside: everything the page's renderer needs from
//! one viewer's `Snapshot`, and the play camera.
//!
//! Built once a match from the same map every browser plays on. Nothing here
//! feeds back into the match: walkers, the eased camera and the mixer live
//! outside the rollback, so a replay costs them nothing. The page reads the
//! finished `Frame` and draws it (`web/play.js`).

use std::sync::Arc;

use serde_json::{json, Value};

use crate::flock::Kind;
use crate::fx::{self, from_int, mul, Fx, HALF_TURN, ONE};
use crate::game::aim::{Ray, BACK};
use crate::game::{Action, BodyRow, FallenRow, Heard, Match, SheepRow, Snapshot, Target, BELL_NOISE};
use crate::ground::Ground;
use crate::map::{Map, Mover};
use crate::models::rig::{Clip, Rig, Walker};
use crate::models::sheep::coat_of;
use crate::models::{character_coat, mesh_of, push_animal, push_shadow, Animal, Library, Model};
use crate::render::geom::{atan2, cm, v3, Affine, V3};
use crate::render::{self, view_values, Camera, Frame, Instance, Outline, BYTES_PER_VALUE, DRAW_VALUES};
use crate::rules::information::in_sight;
use crate::rules::mounds::MoundLook;
use crate::rules::outcome::Outcome;
use crate::rules::tune;
use crate::rules::Role;
use crate::scape::sky;
use crate::scape::view::Hillside;
use crate::scape::world::{MoundDraw, Token, TokenDraw};
use crate::sound::{self, Bank, Gait, HeardSheep, Mixer, Point};

/// The most ticks of walking one frame shows, after a stall.
const MOST_STEPS: u32 = 8;
/// The eye keeps this far off the ground.
const EYE_CLEAR: Fx = cm(35);
/// How high a wall can stop the eye.
const WALL_TOP: Fx = cm(170);
/// The eye keeps this far out of a tree's crown.
const CROWN_CLEAR: Fx = cm(60);
/// How many places along the line the camera tries.
const EYE_STEPS: i32 = 12;
/// The nearest the camera comes.
const EYE_NEAREST: Fx = cm(120);
/// How fast the camera backs out again, each frame.
const EYE_EASE: Fx = cm(6);
/// A body slower than this stands still.
const STILL: Fx = cm(30);
/// How much wider the view opens under Zoomies, percent, and how fast it
/// gets there, percent a frame.
const ZOOMIES_WIDE: i32 = 30;
const WIDE_EASE: i32 = 2;
/// A nameplate's foot sits this far over a body's shoulder: clear of the head.
const PLATE_ABOVE: Fx = cm(70);
/// How fast legs cycle, percent of the pace that plants a foot exactly. Bodies
/// outrun any stride a leg can reach, so feet slide a little rather than blur.
const LEGS_PACE: i32 = 55;
/// A sheep faster than this runs.
const SHEEP_RUN: Fx = cm(520);

/// A teammate's line, shown through walls: sky blue, lighter than the Bell's.
const TEAM_LINE: Outline = Outline {
    color: [cm(30), cm(72), ONE],
    width: 2,
    through: true,
};
/// The other team's line, where this viewer is shown one: orange, apart from
/// the prey's red.
const ENEMY_LINE: Outline = Outline {
    color: [ONE, cm(48), cm(8)],
    width: 2,
    through: true,
};

/// The line on another player's body for this viewer, by team.
fn body_line(viewer: Role, row: &BodyRow) -> Option<Outline> {
    if !row.outline {
        None
    } else if row.role == viewer {
        Some(TEAM_LINE)
    } else {
        Some(ENEMY_LINE)
    }
}
/// A Wolf's red line on the sheep it would eat. It shows only in plain view.
const PREY_LINE: Outline = Outline {
    color: [ONE, cm(10), cm(8)],
    width: 3,
    through: false,
};
/// A Sheepdog's blue line on a sheep wearing a Bell, or about to: dark enough
/// for a cream sheep, and shown through the flock around it.
const BELL_LINE: Outline = Outline {
    color: [cm(8), cm(30), ONE],
    width: 3,
    through: true,
};
/// The line on a mound under the crosshair.
const MOUND_LINE: Outline = Outline {
    color: [ONE, cm(84), cm(40)],
    width: 3,
    through: false,
};

/// A cloaked Wolf as its own team sees it.
const CLOAK_TINT: [Fx; 3] = [cm(58), cm(70), cm(92)];

/// How many puffs one rising scent shows at a time.
const SCENT_PUFFS: i32 = 4;
/// How long a puff takes to rise.
const SCENT_RISE_TICKS: i32 = 150;
/// How high a puff rises, and how far it sways on the way.
const SCENT_RISE: Fx = from_int(3);
const SCENT_SWAY: Fx = cm(30);
/// How long a killed Wolf takes to fall, how far its middle stays off the
/// ground on its side, and how it sinks away at the end.
const FALL_TICKS: i32 = 14;
const FALLEN_LIFT: Fx = cm(30);
const FALLEN_SINK_TICKS: i32 = 40;
const FALLEN_SINK: Fx = cm(70);
/// The burst when a Wolf dies: how many bits, how long, how far and how high
/// they fly, and how many of them, in turn, are blood.
const BURST_BITS: i32 = 18;
const BURST_TICKS: i32 = 42;
const BURST_FAR: Fx = cm(220);
const BURST_HIGH: Fx = cm(170);
const BURST_FROM: Fx = cm(60);
const BURST_BLOOD_EVERY: i32 = 3;
/// Where a carrot's stick starts, up from the feet.
const CARROT_UP: Fx = cm(60);

fn body_model(row: &BodyRow) -> Model {
    match (row.role, row.suited) {
        (Role::Sheepdog, _) => Model::Sheepdog,
        (Role::Wolf, false) => Model::Wolf,
        (Role::Wolf, true) => Model::WolfInSuit,
    }
}

/// A camera's field of view through this much zoom: the view's tangent over
/// the zoom, so what's in the middle shows that many times larger.
fn zoomed_fov(fov: i32, zoom: i32) -> i32 {
    let half = fov / 2;
    let tan = fx::div(fx::sin(half), fx::cos(half));
    2 * atan2(tan / zoom, ONE)
}

/// The line on this sheep for this viewer. `worn_bells` and `belling` reach
/// Sheepdogs alone (FL-7), so a Wolf sees no line of the Bell's. `decoys`
/// too: a Sheepdog sees a Decoy sheep by the Bell's line.
fn sheep_line(s: &Snapshot, id: u16) -> Option<Outline> {
    if s.me.prey == Some(id) {
        Some(PREY_LINE)
    } else if s.me.belling == Some(id) || s.view.worn_bells.contains(&id) || s.decoys.contains(&id) {
        Some(BELL_LINE)
    } else {
        None
    }
}

/// How far a killed Wolf has rolled onto its side: from standing to a quarter
/// turn, then it stays down.
fn fallen_roll(age: i32) -> i32 {
    mul(fx::QUARTER_TURN, fx::ratio(age.min(FALL_TICKS), FALL_TICKS))
}

/// The bits of fur and blood off a Wolf killed here. A pure function of its
/// age, so a rollback has nothing to keep; none once they've landed.
fn burst(f: &FallenRow, at: V3) -> impl Iterator<Item = TokenDraw> {
    let share = fx::ratio(f.age.min(BURST_TICKS), BURST_TICKS);
    let (gone, yaw) = (f.age >= BURST_TICKS, f.yaw as i32);
    (0..if gone { 0 } else { BURST_BITS }).map(move |k| {
        let angle = k * fx::TURN / BURST_BITS + yaw;
        // each bit its own distance and height, from its number alone
        let far = mul(BURST_FAR, share) * (3 + k % 4) / 6;
        let high = mul(BURST_HIGH, mul(4 * share, ONE - share)) * (2 + (k * 5) % 3) / 4;
        let way = v3(
            mul(fx::sin(angle), far),
            BURST_FROM + high - mul(BURST_FROM, share),
            mul(fx::cos(angle), far),
        );
        let size = ONE - share * 2 / 3;
        TokenDraw {
            token: if k % BURST_BLOOD_EVERY == 0 {
                Token::Blood
            } else {
                Token::Fur
            },
            place: Affine::translate(at.add(way)).then(&Affine::scale(v3(size, size, size))),
        }
    })
}

/// Whether a noise reaches a listener: within sight, or a Bell, which rings
/// for everyone.
fn carries(h: &Heard, range: Fx, ear: (Fx, Fx)) -> bool {
    h.name == BELL_NOISE || in_sight(range, ear, (h.x, h.z))
}

/// The puffs of one rising scent. A pure function of the tick, so a rollback
/// has nothing to keep; each scent runs on from where it stands.
fn scent_puffs(at: V3, tick: u32) -> impl Iterator<Item = TokenDraw> {
    let start = tick.wrapping_add((at.x ^ at.z) as u32);
    (0..SCENT_PUFFS).map(move |k| {
        let gap = (k * SCENT_RISE_TICKS / SCENT_PUFFS) as u32;
        let rise = (start.wrapping_add(gap) % SCENT_RISE_TICKS as u32) as i32;
        let share = fx::ratio(rise, SCENT_RISE_TICKS);
        let sway = mul(
            SCENT_SWAY,
            fx::sin(mul(share, fx::TURN) + k * fx::TURN / SCENT_PUFFS),
        );
        let size = ONE - share * 3 / 4;
        let up = v3(sway, mul(SCENT_RISE, share), 0);
        let place = Affine::translate(at.add(up)).then(&Affine::scale(v3(size, size, size)));
        TokenDraw {
            token: Token::Scent,
            place,
        }
    })
}

fn sheep_model(kind: Kind) -> Model {
    match kind {
        Kind::Ewe => Model::Ewe,
        Kind::Ram => Model::Ram,
        Kind::Lamb => Model::Lamb,
    }
}

/// The clip a body wants.
pub fn body_clip(b: &BodyRow) -> Clip {
    let moving = b.speed > STILL;
    match (b.action, b.grounded, b.crouched, moving, b.sprinting) {
        (Action::Digging, ..) => Clip::Dig,
        (Action::Snapping, ..) => Clip::Snap,
        (Action::Lunging, ..) => Clip::Run,
        (_, false, ..) => Clip::Jump,
        (_, _, true, true, _) => Clip::Stalk,
        (_, _, true, false, _) => Clip::Crouch,
        (_, _, _, true, true) => Clip::Run,
        (_, _, _, true, false) => Clip::Walk,
        _ => Clip::Idle,
    }
}

/// The clip a sheep wants.
pub fn sheep_clip(s: &SheepRow) -> Clip {
    match (s.grazing, s.speed > STILL, s.speed > SHEEP_RUN) {
        (_, _, true) => Clip::Run,
        (true, true, _) => Clip::Drift,
        (true, false, _) => Clip::Graze,
        (false, true, _) => Clip::Walk,
        (false, false, _) => Clip::Idle,
    }
}

fn point(at: V3) -> Point {
    Point {
        x: at.x,
        y: at.y,
        z: at.z,
    }
}

/// Everything a viewer's page needs for one match.
pub struct Scene {
    hill: Hillside,
    map: Arc<Map>,
    seat: usize,
    /// One walker per sheep and per seat, by id.
    sheep: Vec<Walker>,
    bodies: Vec<Walker>,
    seen_tick: u32,
    /// Where the viewer last stood, for the wait after a death.
    stood: V3,
    /// How much wider the view is right now, percent.
    wide: i32,
    /// How far back the eased camera is.
    back: Fx,
    frame: Frame,
    mixer: Option<Mixer>,
    bank: Option<Bank>,
    sound: Value,
    /// The win tune has gone out.
    sang: bool,
    /// Each seat's name as the lobby shows it, by seat.
    names: Vec<String>,
    /// This frame's nameplates, for the page to place.
    plates: Value,
}

/// An open scene and the seed it was built for, held by `App`. A clone of
/// the app holds none: the page opens one when it needs it.
#[derive(Default)]
pub struct Slot(pub Option<(u64, Scene)>);

impl Clone for Slot {
    fn clone(&self) -> Slot {
        Slot(None)
    }
}

impl std::fmt::Debug for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Slot({:?})", self.0.as_ref().map(|s| s.0))
    }
}

impl Scene {
    pub fn new(game: &Match, seed: u64, seat: usize) -> Scene {
        let map = game.shared_map();
        let mut hill = Hillside::of_match(game.map().clone(), seed);
        hill.bake();
        let bank = sound::bank().ok();
        let flock = &game.world.flock.sheep;
        let phase = |i: usize| (i as i32 * 41) % ONE;
        Scene {
            hill,
            map,
            seat,
            // a walker for every id, Decoy sheep's too: `rules::decoy`
            sheep: (0..flock.len() + crate::tune::MAX_PLAYERS)
                .map(|i| Walker::new(Clip::Idle, phase(i)))
                .collect(),
            bodies: vec![Walker::new(Clip::Idle, 0); crate::online::SEATS],
            seen_tick: 0,
            stood: v3(0, 0, 0),
            wide: 0,
            back: BACK,
            frame: Frame::default(),
            mixer: bank.clone().and_then(|b| Mixer::new(b, seed).ok()),
            bank,
            sound: json!({"cues": [], "moves": [], "loops": []}),
            sang: false,
            names: Vec::new(),
            plates: json!([]),
        }
    }

    pub fn numbers(&self) -> String {
        self.hill.numbers()
    }

    pub fn hill(&self) -> &Hillside {
        &self.hill
    }

    pub fn resize(&mut self, css_w: i32, css_h: i32, dpr_pct: i32) -> [i32; 2] {
        self.hill.resize(css_w, css_h, dpr_pct)
    }

    pub fn frame(&self) -> &Frame {
        &self.frame
    }

    /// What the page plays this frame.
    pub fn sound(&self) -> String {
        self.sound.to_string()
    }

    /// The seats' names, by seat, for the nameplates.
    pub fn set_names(&mut self, names: Vec<String>) {
        self.names = names;
    }

    /// This frame's nameplates: each one's name and where its foot goes, as
    /// CSS percents of the view.
    pub fn plates(&self) -> String {
        self.plates.to_string()
    }

    /// A plate over each body the snapshot gives one, on screen this frame.
    fn place_plates(&self, s: &Snapshot, cam: Camera) -> Value {
        let size = self.hill.size();
        let percent = |at: i32, of: i32| {
            let p = at * 10_000 / of.max(1);
            format!("{}.{:02}%", p / 100, p % 100)
        };
        let rows: Vec<Value> = s
            .bodies
            .iter()
            .filter(|b| s.plates.contains(&b.seat))
            .filter_map(|b| {
                let name = self.names.get(b.seat as usize)?;
                let shoulder = if b.crouched {
                    b.role.breed().shoulder_crouched
                } else {
                    b.role.breed().shoulder
                };
                let top = v3(b.x, b.y + shoulder + PLATE_ABOVE, b.z);
                let (x, y) = cam.screen(size, top)?;
                let on = (0..size[0]).contains(&x) && (0..size[1]).contains(&y);
                // the plate takes its outline's color: the page's class for it
                let side = if b.role == s.me.role { "ally" } else { "enemy" };
                on.then(|| {
                    json!({ "name": name, "side": side, "left": percent(x, size[0]), "top": percent(y, size[1]) })
                })
            })
            .collect();
        Value::Array(rows)
    }

    /// Whether the camera's eye can stand here.
    fn clear(&self, p: V3) -> bool {
        let ground = self.hill.scape().field.height_at(p.x, p.z);
        if p.y < ground + EYE_CLEAR {
            return false;
        }
        let in_crown = |t: &crate::map::props::Tree| {
            let reach = t.crown + CROWN_CLEAR;
            let (dx, dz) = (t.x - p.x, t.z - p.z);
            if dx.abs() > reach || dz.abs() > reach {
                return false;
            }
            let dy = self.map.height(t.x, t.z) + t.height - t.crown - p.y;
            fx::length(fx::length(dx, dz), dy) < reach
        };
        if self.map.trees.iter().any(in_crown) {
            return false;
        }
        let thin = cm(10);
        p.y >= ground + WALL_TOP || self.map.push_out_as(p.x, p.z, thin, Mover::Sheep) == (p.x, p.z)
    }

    /// How far back along the line the eye can stand: the farthest place
    /// before anything gets in the way.
    fn room_behind(&self, ray: &Ray) -> Fx {
        let mut room = EYE_NEAREST;
        for k in 1..=EYE_STEPS {
            let back = EYE_NEAREST + (BACK - EYE_NEAREST) * k / EYE_STEPS;
            if !self.clear(ray.eye(back)) {
                break;
            }
            room = back;
        }
        room
    }

    /// The camera for this look. It comes in at once and eases back out.
    fn camera(&mut self, at: V3, aim: (u16, i16)) -> Camera {
        let ray = Ray::new(at, aim.0, aim.1);
        let room = self.room_behind(&ray);
        self.back = (self.back + EYE_EASE).min(room);
        Camera::along(ray.eye(self.back), ray.way)
    }

    fn walk(&mut self, s: &Snapshot) -> u32 {
        let steps = s.tick.wrapping_sub(self.seen_tick).min(MOST_STEPS);
        self.seen_tick = s.tick;
        for row in &s.sheep {
            if let Some(w) = self.sheep.get_mut(row.id as usize) {
                let rig = sheep_model(row.kind).rig();
                for _ in 0..steps {
                    w.step(rig, sheep_clip(row), legs_pace(row.speed));
                }
            }
        }
        for row in &s.bodies {
            let rig: Rig = body_model(row).rig();
            for _ in 0..steps {
                self.bodies[row.seat as usize].step(rig, body_clip(row), legs_pace(row.speed));
            }
        }
        steps
    }

    fn push_sheep(&self, b: &mut render::Batch, eye: V3, s: &Snapshot) {
        let field = &self.hill.scape().field;
        let lib: &Library = self.hill.library();
        for row in &s.sheep {
            let model = sheep_model(row.kind);
            let coat = coat_of(row.id as i32);
            let [nx, ny, nz] = field.normal_near(row.x, row.z);
            let walker = &self.sheep[row.id as usize];
            push_animal(
                b,
                eye,
                &Animal {
                    model,
                    at: v3(row.x, field.height_at(row.x, row.z) + row.hop, row.z),
                    yaw: atan2(row.face_x, row.face_z),
                    up: v3(nx, ny, nz),
                    size: if row.kind == Kind::Lamb { ONE } else { row.size },
                    pose: walker.pose(model.rig(), &lib.poses),
                    coat: coat.wool,
                    skin: coat.skin,
                    outline: sheep_line(s, row.id),
                    bell: s.view.worn_bells.contains(&row.id),
                },
            );
        }
    }

    fn push_bodies(&self, b: &mut render::Batch, eye: V3, s: &Snapshot) {
        let field = &self.hill.scape().field;
        let lib: &Library = self.hill.library();
        for row in &s.bodies {
            let mine = row.seat as usize == self.seat;
            if mine && s.me.scoped {
                continue;
            }
            let model = body_model(row);
            let (coat, skin) = if row.cloaked {
                (CLOAK_TINT, CLOAK_TINT)
            } else {
                character_coat(model)
            };
            let [nx, ny, nz] = field.normal_near(row.x, row.z);
            let walker = &self.bodies[row.seat as usize];
            push_animal(
                b,
                eye,
                &Animal {
                    model,
                    at: v3(row.x, row.y, row.z),
                    yaw: HALF_TURN - row.yaw as i32,
                    up: if row.grounded { v3(nx, ny, nz) } else { V3::UP },
                    size: ONE,
                    pose: walker.pose(model.rig(), &lib.poses),
                    coat,
                    skin,
                    outline: body_line(s.me.role, row).filter(|_| !mine),
                    bell: false,
                },
            );
        }
    }

    fn push_carcasses(&self, b: &mut render::Batch, s: &Snapshot) {
        let field = &self.hill.scape().field;
        for c in &s.carcasses {
            let [nx, ny, nz] = field.normal_near(c.x, c.z);
            let at = v3(c.x, field.height_at(c.x, c.z), c.z);
            let place = render::stand(at, 0, v3(nx, ny, nz), ONE);
            b.push(mesh_of(Model::Carcass, 0), &Instance::still(place), None);
        }
    }

    /// Wolves and Sheepdogs just killed: down on their sides, whatever they
    /// wore.
    fn push_fallen(&self, b: &mut render::Batch, s: &Snapshot) {
        let field = &self.hill.scape().field;
        let lib: &Library = self.hill.library();
        for f in &s.fallen {
            let model = match f.role {
                Role::Wolf => Model::Wolf,
                Role::Sheepdog => Model::Sheepdog,
            };
            let (coat, skin) = character_coat(model);
            let pose = Walker::new(Clip::Idle, 0).pose(model.rig(), &lib.poses);
            let [nx, ny, nz] = field.normal_near(f.x, f.z);
            let roll = fallen_roll(f.age);
            let lift = mul(FALLEN_LIFT, fx::ratio(roll, fx::QUARTER_TURN));
            let left = tune::KILL_BODY_TICKS - f.age;
            let sink = mul(
                FALLEN_SINK,
                fx::ratio((FALLEN_SINK_TICKS - left).max(0), FALLEN_SINK_TICKS),
            );
            let at = v3(f.x, field.height_at(f.x, f.z), f.z);
            let up = v3(nx, ny, nz);
            let yaw = HALF_TURN - f.yaw as i32;
            let place = render::stand(at.add(v3(0, lift - sink, 0)), yaw, up, ONE).then(&Affine::rot_z(roll));
            let i = Instance {
                place,
                pose,
                coat,
                skin,
            };
            b.push(mesh_of(model, 0), &i, None);
            push_shadow(b, model, at, yaw, up, ONE);
        }
    }

    /// What powerups put in the world this frame.
    fn tokens(&self, s: &Snapshot) -> Vec<TokenDraw> {
        let field = &self.hill.scape().field;
        let on_ground = |x: Fx, z: Fx| v3(x, field.height_at(x, z), z);
        let aura = s.carcasses.iter().filter(|c| c.aura).map(|c| on_ground(c.x, c.z));
        let scents = s.scents.iter().map(|[x, z]| on_ground(*x, *z));
        let mut tokens: Vec<TokenDraw> = aura
            .chain(scents)
            .flat_map(|at| scent_puffs(at, s.tick))
            .collect();
        for f in &s.fallen {
            tokens.extend(burst(f, on_ground(f.x, f.z)));
        }
        for row in s.bodies.iter().filter(|b| b.carrot) {
            let at = v3(row.x, row.y + CARROT_UP, row.z);
            let yaw = HALF_TURN - row.yaw as i32;
            tokens.push(TokenDraw {
                token: Token::Carrot,
                place: render::stand(at, yaw, V3::UP, ONE),
            });
        }
        tokens
    }

    fn mounds(&self, s: &Snapshot) -> Vec<MoundDraw> {
        let field = &self.hill.scape().field;
        let under = match s.me.target {
            Some(Target::Mound(i)) => Some(i as usize),
            _ => None,
        };
        s.mound_spots
            .iter()
            .zip(&s.view.mounds)
            .enumerate()
            .map(|(i, ([x, z], look))| {
                let [nx, ny, nz] = field.normal_near(*x, *z);
                MoundDraw {
                    at: v3(*x, field.height_at(*x, *z), *z),
                    up: v3(nx, ny, nz),
                    dug: *look == MoundLook::DugUp,
                    outline: (under == Some(i)).then_some(MOUND_LINE),
                }
            })
            .collect()
    }

    /// The frame for this viewer, looking along `aim`. `aim` is the page's
    /// newest, so the camera answers the mouse before the next tick does.
    pub fn build(&mut self, game: &Match, aim: (u16, i16), zoom: i32) {
        let Some(s) = game.snapshot_for(self.seat) else {
            return;
        };
        self.walk(&s);
        let mine = s.bodies.iter().find(|b| b.seat as usize == self.seat);
        if let Some(b) = mine {
            self.stood = v3(b.x, b.y, b.z);
        }
        let zooming = s.me.running.iter().any(|r| r.icon == "zoomies");
        let wide = if zooming { ZOOMIES_WIDE } else { 0 };
        self.wide += (wide - self.wide).clamp(-WIDE_EASE, WIDE_EASE);
        let cam = if s.me.scoped {
            scope_camera(self.stood, aim, zoom)
        } else {
            let mut cam = self.camera(self.stood, aim);
            cam.fov += cam.fov * self.wide / 100;
            cam
        };
        self.hill.set_view(cam, s.tick);

        let mut b = self.hill.library().batch();
        self.push_sheep(&mut b, cam.eye, &s);
        self.push_bodies(&mut b, cam.eye, &s);
        self.push_carcasses(&mut b, &s);
        self.push_fallen(&mut b, &s);
        let (mut instances, rows, mut outlines) = b.finish(&self.hill.library().ranges);
        let scape = self.hill.scape();
        let (moving, mut moving_rows, mut moving_lines) =
            scape.moving_for(&self.mounds(&s), &self.tokens(&s), s.tick);
        let shift = (instances.len() * BYTES_PER_VALUE) as u32;
        for row in moving_rows
            .chunks_mut(DRAW_VALUES)
            .chain(moving_lines.chunks_mut(DRAW_VALUES))
        {
            row[5] += shift;
        }
        instances.extend_from_slice(&moving);
        outlines.extend_from_slice(&moving_lines);
        let mut draws = scape.rows_for(cam.eye, cam.fwd);
        draws.extend_from_slice(&moving_rows);
        draws.extend_from_slice(&rows);
        self.frame = Frame {
            view: view_values(
                &cam,
                self.hill.size(),
                &sky::dimmed(sky::at(s.darkness), s.view.light),
                false,
            ),
            instances,
            draws,
            outlines,
        };
        self.plates = self.place_plates(&s, cam);
        self.hear(game, &s, cam);
    }

    /// The camera is the listener.
    fn hear(&mut self, game: &Match, s: &Snapshot, cam: Camera) {
        let field = &self.hill.scape().field;
        let flock: Vec<HeardSheep> = s
            .sheep
            .iter()
            .map(|row| HeardSheep {
                at: point(v3(row.x, field.height_at(row.x, row.z), row.z)),
                lamb: row.kind == Kind::Lamb,
                moving: row.speed > STILL,
            })
            .collect();
        let tick = s.tick as i64;
        let range = s.view.sight_range;
        let noises: Vec<_> = game
            .heard()
            .iter()
            .filter(|h| carries(h, range, (cam.eye.x, cam.eye.z)))
            .map(|h| (*h, field.height_at(h.x, h.z)))
            .collect();
        let ringing: Vec<(i64, Point)> = s
            .sheep
            .iter()
            .filter(|r| s.view.ringing_bells.contains(&r.id))
            .map(|r| (r.id as i64, point(v3(r.x, field.height_at(r.x, r.z), r.z))))
            .collect();
        let tune = self.tune(s);
        let Some(mixer) = &mut self.mixer else {
            return;
        };
        mixer.listen(point(cam.eye), cam.fwd.x, cam.fwd.z);
        let _ = mixer.flock(&flock, tick);
        for row in &s.bodies {
            let gait = match (row.speed > STILL && row.grounded, row.sprinting) {
                (false, _) => Gait::Still,
                (true, false) => Gait::Run,
                (true, true) => Gait::Sprint,
            };
            let at = point(v3(row.x, row.y, row.z));
            let _ = mixer.mover(row.seat as usize, at, gait, row.winded, tick);
        }
        for (h, y) in noises {
            let at = point(v3(h.x, y, h.z));
            let _ = mixer.event(h.name, h.who as i64, Some(at), h.tick as i64);
        }
        for (sheep, at) in ringing {
            mixer.source(sheep, at);
        }
        if let Ok(mut frame) = mixer.frame(tick) {
            if let Some(tune) = tune {
                frame["tune"] = tune;
            }
            self.sound = frame;
        }
    }

    /// The winners' tune, once, on the frame the match ends.
    fn tune(&mut self, s: &Snapshot) -> Option<Value> {
        let outcome: Outcome = s.outcome?;
        if std::mem::replace(&mut self.sang, true) {
            return None;
        }
        let bank = self.bank.as_ref()?;
        let id = sound::win_tune(bank, outcome.winner() == Role::Sheepdog).ok()?;
        bank.tune(&id).ok()
    }

    /// The nearest the sim's own height comes to what's drawn: tests only.
    #[cfg(test)]
    fn back(&self) -> Fx {
        self.back
    }
}

/// The view through Dognoculars: from the point the camera turns on, along
/// the crosshair's line.
/// The speed the leg cycle runs at for a body moving at `speed`.
fn legs_pace(speed: Fx) -> Fx {
    speed * LEGS_PACE / 100
}

fn scope_camera(at: V3, aim: (u16, i16), zoom: i32) -> Camera {
    let ray = Ray::new(at, aim.0, aim.1);
    let mut cam = Camera::along(ray.pivot, ray.way);
    cam.fov = zoomed_fov(cam.fov, zoom);
    cam
}

/// How far a percent `f` of 1 is: 0 to 100.
pub fn percent(f: Fx) -> i32 {
    mul(f.clamp(0, ONE), from_int(100)) >> fx::SHIFT
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::aim::{BACK, REST_PITCH};
    use crate::lobby::{MatchStart, Role as Pick, StartSeat};
    use crate::render::FRAME_SOURCE;
    use crate::rules::setup::Settings;

    fn scene_and_game() -> (Scene, Match) {
        let seat = |k: u32, role| StartSeat {
            seat: k,
            role,
            person: format!("p{k}"),
            name: format!("p{k}"),
        };
        let start = MatchStart {
            seed: 3,
            seats: vec![seat(0, Pick::Wolf), seat(1, Pick::Sheepdog)],
            settings: Settings {
                sheep: 20,
                eats_required: 5,
                ..Settings::default()
            },
        };
        let game = Match::new(&start, Arc::new(Map::generate(3)));
        (Scene::new(&game, 3, 0), game)
    }

    #[test]
    fn a_plate_names_the_sheepdog_ahead_of_a_wolf_and_none_names_the_wolf() {
        let (mut scene, mut game) = scene_and_game();
        let (x, z) = (game.world.bodies[0].x, game.world.bodies[0].z);
        let map = game.shared_map();
        let dog = &mut game.world.bodies[1];
        (dog.x, dog.z) = (x, z - from_int(8));
        dog.settle(&*map);
        scene.resize(1920, 1080, 100);
        scene.set_names(vec!["Wolfie".into(), "Rex".into()]);
        scene.build(&game, (0, crate::game::aim::REST_PITCH), 3);
        let plates: Value = serde_json::from_str(&scene.plates()).unwrap();
        let names: Vec<_> = plates
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["name"].clone())
            .collect();
        assert_eq!(names, vec![json!("Rex")]);
        // the Wolf sees a Sheepdog: the other team's color
        assert_eq!(plates[0]["side"], json!("enemy"));
    }

    #[test]
    fn a_frame_draws_the_flock_and_every_row_fits_what_it_holds() {
        let (mut scene, game) = scene_and_game();
        scene.build(&game, game.first_look(0), 3);
        let f = scene.frame();
        assert!(!f.view.is_empty());
        let shown: u32 = f
            .draws
            .chunks(DRAW_VALUES)
            .filter(|r| r[4] == FRAME_SOURCE)
            .map(|r| {
                let end = r[5] as usize + r[6] as usize * render::INSTANCE_VALUES * BYTES_PER_VALUE;
                assert!(end <= f.instances.len() * BYTES_PER_VALUE);
                r[6]
            })
            .sum();
        // the two bodies and a clump in sight, with their shadows; the
        // flock starts in clumps, so some are out of sight
        assert!(shown >= 20, "{shown}");
    }

    #[test]
    fn a_hill_behind_pulls_the_camera_in_on_the_crosshairs_line() {
        let (mut scene, game) = scene_and_game();
        let at = v3(
            game.world.bodies[0].x,
            game.world.bodies[0].y,
            game.world.bodies[0].z,
        );
        let look = (game.first_look(0).0, REST_PITCH);
        let ray = Ray::new(at, look.0, look.1);
        // the rest of the look clears the ground
        assert_eq!(scene.room_behind(&ray), BACK);
        // looking up from under the ground, the eye can't stand behind
        let low = Ray::new(at.sub(v3(0, from_int(2), 0)), look.0, 2700);
        assert!(scene.room_behind(&low) < BACK);
        scene.back = BACK;
        let cam = scene.camera(at.sub(v3(0, from_int(2), 0)), (look.0, 2700));
        assert!(scene.back() < BACK);
        assert_eq!(cam.fwd.norm(), low.way.norm());
    }

    #[test]
    fn the_eye_keeps_a_little_clear_of_the_ground() {
        let (scene, game) = scene_and_game();
        let body = &game.world.bodies[0];
        let (x, z) = (body.x, body.z);
        let ground = scene.hill.scape().field.height_at(x, z);
        assert!(scene.clear(v3(x, ground + cm(36), z)));
        assert!(!scene.clear(v3(x, ground + cm(34), z)));
    }

    #[test]
    fn legs_cycle_at_a_little_over_half_the_ground_speed() {
        // 55% of 10 m/s
        assert_eq!(legs_pace(from_int(10)), from_int(11) / 2);
        assert_eq!(legs_pace(0), 0);
    }

    #[test]
    fn three_times_the_zoom_narrows_the_view_to_about_eighteen_degrees() {
        // tan 26 is 0.4877, a third of it 0.1626, whose angle is 9.23 degrees:
        // 18.47 degrees across, which is 3362 of 65536 turns; `atan2` is good
        // to a tenth of a degree, twice over here, which is 36 of them
        let fov = zoomed_fov(fx::TURN * 52 / 360, 3);
        assert!((fov - 3362).abs() < 36, "{fov}");
    }

    #[test]
    fn the_spyglass_looks_down_the_crosshairs_line_from_the_body() {
        let at = v3(from_int(3), 0, from_int(4));
        let cam = scope_camera(at, (9000, -2000), 3);
        let ray = Ray::new(at, 9000, -2000);
        assert_eq!(cam.eye, ray.pivot);
        assert_eq!(cam.fwd, ray.way);
    }

    #[test]
    fn a_bell_carries_past_sight_and_other_noises_do_not() {
        let far = Heard {
            tick: 0,
            name: "dig",
            who: 0,
            x: from_int(300),
            z: 0,
        };
        let range = from_int(35);
        assert!(!carries(&far, range, (0, 0)));
        let bell = Heard {
            name: BELL_NOISE,
            ..far
        };
        assert!(carries(&bell, range, (0, 0)));
    }

    #[test]
    fn a_scent_rises_shrinking_and_starts_over() {
        let at = v3(0, 0, 0);
        let puffs = |tick| scent_puffs(at, tick).collect::<Vec<_>>();
        assert_eq!(puffs(0).len(), 4);
        // the first puff, 75 of 150 ticks up, is halfway to its 3 m
        let high = puffs(75)[0].place.t.y;
        assert_eq!(fx::round(high * 2), 3);
        assert!(puffs(75)[0].place.c1.y < puffs(0)[0].place.c1.y);
        assert_eq!(puffs(150), puffs(0));
    }

    #[test]
    fn sniff_snoff_and_a_carrot_put_their_tokens_in_the_world() {
        let (scene, game) = scene_and_game();
        let mut s = game.snapshot_for(1).unwrap();
        assert!(scene.tokens(&s).is_empty());
        s.carcasses.push(crate::game::CarcassRow {
            x: from_int(10),
            z: from_int(10),
            aura: true,
        });
        s.carcasses.push(crate::game::CarcassRow {
            x: from_int(12),
            z: from_int(10),
            aura: false,
        });
        s.scents.push([from_int(20), from_int(10)]);
        s.bodies[0].carrot = true;
        let tokens = scene.tokens(&s);
        // four puffs over the one carcass with an aura, four over the scent
        assert_eq!(tokens.len(), 9);
        assert_eq!(tokens[8].token, Token::Carrot);
    }

    #[test]
    fn a_sheep_with_a_bell_or_about_to_have_one_gets_the_bell_s_line() {
        let (_, m) = scene_and_game();
        let mut s = m.snapshot_for(0).unwrap();
        assert_eq!(sheep_line(&s, 3), None);
        s.view.worn_bells = vec![3];
        assert_eq!(sheep_line(&s, 3), Some(BELL_LINE));
        assert_eq!(sheep_line(&s, 4), None);
        s.me.belling = Some(4);
        assert_eq!(sheep_line(&s, 4), Some(BELL_LINE));
        s.me.prey = Some(4);
        assert_eq!(sheep_line(&s, 4), Some(PREY_LINE));
    }

    #[test]
    fn teammates_line_blue_and_the_other_team_orange() {
        let dog = BodyRow {
            seat: 1,
            role: Role::Sheepdog,
            x: 0,
            y: 0,
            z: 0,
            yaw: 0,
            pitch: 0,
            crouched: false,
            sprinting: false,
            grounded: true,
            speed: 0,
            action: Action::Moving,
            winded: false,
            outline: true,
            suited: false,
            cloaked: false,
            carrot: false,
            frenzy: false,
        };
        assert_eq!(body_line(Role::Sheepdog, &dog), Some(TEAM_LINE));
        assert_eq!(body_line(Role::Wolf, &dog), Some(ENEMY_LINE));
        let unlined = BodyRow {
            outline: false,
            ..dog
        };
        assert_eq!(body_line(Role::Wolf, &unlined), None);
        assert_ne!(TEAM_LINE.color, ENEMY_LINE.color);
    }

    #[test]
    fn a_killed_wolf_rolls_onto_its_side_and_stays_down() {
        assert_eq!(fallen_roll(0), 0);
        // half of 14 ticks is half of a quarter turn
        assert_eq!(fallen_roll(7), fx::QUARTER_TURN / 2);
        assert_eq!(fallen_roll(14), fx::QUARTER_TURN);
        assert_eq!(fallen_roll(200), fx::QUARTER_TURN);
    }

    #[test]
    fn a_kill_throws_up_fur_and_blood_and_they_land() {
        let f = |age| FallenRow {
            role: Role::Wolf,
            x: 0,
            z: 0,
            yaw: 0,
            age,
        };
        let bits: Vec<_> = burst(&f(10), v3(0, 0, 0)).collect();
        assert_eq!(bits.len(), 18);
        // bits 0, 3, 6 and so on: 6 of the 18
        assert_eq!(bits.iter().filter(|b| b.token == Token::Blood).count(), 6);
        assert_eq!(burst(&f(42), v3(0, 0, 0)).count(), 0);
    }

    #[test]
    fn a_body_s_clip_follows_what_it_is_doing() {
        let mut b = BodyRow {
            seat: 0,
            role: Role::Wolf,
            x: 0,
            y: 0,
            z: 0,
            yaw: 0,
            pitch: 0,
            crouched: false,
            sprinting: false,
            grounded: true,
            speed: 0,
            action: Action::Moving,
            winded: false,
            outline: false,
            suited: false,
            cloaked: false,
            carrot: false,
            frenzy: false,
        };
        assert_eq!(body_clip(&b), Clip::Idle);
        b.speed = from_int(4);
        assert_eq!(body_clip(&b), Clip::Walk);
        b.sprinting = true;
        assert_eq!(body_clip(&b), Clip::Run);
        b.crouched = true;
        assert_eq!(body_clip(&b), Clip::Stalk);
        b.grounded = false;
        b.crouched = false;
        assert_eq!(body_clip(&b), Clip::Jump);
        b.action = Action::Digging;
        assert_eq!(body_clip(&b), Clip::Dig);
        b.action = Action::Lunging;
        assert_eq!(body_clip(&b), Clip::Run);
    }
}
