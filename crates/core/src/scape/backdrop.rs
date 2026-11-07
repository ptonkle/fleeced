//! The scene behind the menus: the real hillside at a fixed seed and a fixed
//! golden sunset, a slow camera, and in a lobby every seat's character on the
//! grass. It is scenery for the page and nothing else: it holds no match, and
//! no rule or checksum reads it.
//!
//! One line runs through it: the title's camera stands near `SPOT`, looking
//! along `HEADING`, and the lobby's lineup and a winning team stand on the
//! nearest open ground to it (`podium`), looking the same way toward the same
//! hills.

use serde::Deserialize;

use crate::dock::Dock;
use crate::fx::{self, from_int, mul, Fx, ONE, TURN};
use crate::map::solids::{polygon_contains, Part};
use crate::map::Map;
use crate::models::{lineup, victors, Model, Stand};
use crate::render::geom::{cm, deg, v3, V3};
use crate::render::Camera;
use crate::view::LineupSeat;

use super::field::Field;
use super::hills::REACH;

/// The map the menus stand on, and the sun's place, percent of the way to
/// night.
pub const SEED: i32 = 5;
pub const PHASE: i32 = 50;
/// Sheep the map scatters, of which the nearest to `FLOCK_AHEAD` stay.
pub const SCATTERED: i32 = 800;
pub const FLOCK: usize = 34;
/// How far ahead of the lineup the flock grazes, and how far to the side.
pub const FLOCK_AHEAD: Fx = from_int(34);
pub const FLOCK_SIDE: Fx = from_int(-16);
/// No sheep grazes nearer the line than this, in the way of the view.
pub const FLOCK_PAST: Fx = from_int(9);

/// The menus' frame cap, microseconds: a title screen needs no more than 30
/// frames a second.
pub const FRAME_US: i64 = 33_333;
/// The drawing buffer's most pixels to a CSS pixel, percent.
pub const DENSITY_PCT: i32 = 100;

/// Where the title looks from, and the way everything looks.
pub const SPOT: (Fx, Fx) = (from_int(211), from_int(59));
pub const HEADING: i32 = deg(-82);

const TITLE_BACK: Fx = from_int(16);
const TITLE_UP: Fx = cm(420);
const TITLE_FOV: i32 = deg(50);
/// The point the title view holds as the camera sways round it.
const TITLE_AIM: Fx = from_int(70);
const TITLE_AIM_UP: Fx = from_int(7);
const TITLE_SWAY: Fx = cm(700);
const TITLE_BREATH: Fx = cm(300);

const LOBBY_UP: Fx = cm(260);
const LOBBY_FOV: i32 = deg(32);
const LOBBY_AIM_UP: Fx = cm(130);
const LOBBY_SWAY: Fx = cm(80);
const LOBBY_NEAREST: Fx = from_int(11);
const LOBBY_FARTHEST: Fx = from_int(44);
/// Past the outermost feet, so a character clears the panel.
const LINEUP_MARGIN: Fx = cm(170);
/// How wide the lineup spreads across the open middle, of the gallery's arc.
const LINEUP_TIGHT: Fx = ONE * 3 / 4;
/// The row behind your own stands this much farther back, so a character
/// straight behind the plinth still shows over it.
const LINEUP_DEEP: Fx = ONE * 3 / 2;
/// A character straight behind the plinth steps this far aside.
const LINEUP_ASIDE: Fx = cm(90);

/// The winners stand where this much of the map is open: from the camera,
/// this far behind the line, to this far past it, this far to each side.
const WIN_BEHIND: Fx = from_int(26);
const WIN_PAST: Fx = from_int(6);
const WIN_SIDE: Fx = from_int(12);
/// How near anything solid may come to the open part.
const WIN_ROOM: Fx = from_int(4);
/// How far anything solid keeps from each of those spots.
const WIN_CLEAR: Fx = from_int(2);
/// How far the open part may rise or fall from the line's own ground.
const WIN_SLOPE: Fx = cm(160);
/// The search for that place: rings of squares round `SPOT`.
const WIN_STEP: Fx = from_int(2);
const WIN_RINGS: i32 = 60;
const WIN_UP: Fx = cm(130);
const WIN_AIM_UP: Fx = cm(150);
const WIN_SWAY: Fx = cm(40);
/// How far toward the camera the ground keeps clear of flat stones.
const WIN_STONES: Fx = from_int(8);
const WIN_NEAREST: Fx = from_int(8);
/// Past the outermost feet: the team stands close together.
const WIN_MARGIN: Fx = cm(110);

/// One slow sway there and back, in ticks.
const SWAY_TICKS: u32 = 7200;
/// The camera covers this share of the way to its shot each tick.
const EASE_SHIFT: i32 = 5;
const FAR: Fx = REACH + from_int(300);
const NEAR: Fx = cm(30);

/// What the page says is on screen.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Stage {
    pub width: i32,
    pub height: i32,
    pub dock: Dock,
    pub lineup: Vec<LineupSeat>,
    /// The winners' screen: the lineup is a winning team.
    #[serde(default)]
    pub cheer: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shot {
    eye: V3,
    aim: V3,
    fov: i32,
}

pub struct Backdrop {
    stage: Stage,
    /// Where the lineup and a winning team stand (`podium`).
    podium: (Fx, Fx),
    stands: Vec<Stand>,
    shot: Shot,
    clock: u32,
    /// The lineup's turn: characters spin even under reduced motion, which
    /// holds the camera still.
    spin: u32,
    still: bool,
    next_frame: i64,
}

fn along() -> (Fx, Fx) {
    (fx::sin(HEADING), fx::cos(HEADING))
}

/// A point on the ground plan, `ahead` along the view from `spot` and `side`
/// to its right.
fn ground_from(spot: (Fx, Fx), ahead: Fx, side: Fx) -> (Fx, Fx) {
    let (dx, dz) = along();
    (
        spot.0 + mul(dx, ahead) - mul(dz, side),
        spot.1 + mul(dz, ahead) + mul(dx, side),
    )
}

/// Where the flock grazes, seen from where the lineup stands.
pub fn flock_at(spot: (Fx, Fx)) -> (Fx, Fx) {
    ground_from(spot, FLOCK_AHEAD, FLOCK_SIDE)
}

/// How far along the view from `spot` a point lies.
pub fn ahead_of(spot: (Fx, Fx), x: Fx, z: Fx) -> Fx {
    let (dx, dz) = along();
    mul(x - spot.0, dx) + mul(z - spot.1, dz)
}

/// A point on the ground plan, from `SPOT`.
pub fn ground_at(ahead: Fx, side: Fx) -> (Fx, Fx) {
    ground_from(SPOT, ahead, side)
}

/// Is the ground before and around `spot` clear for the winners and the
/// camera: on the map, nothing solid in it, and about level?
fn open_at(map: &Map, field: &Field, spot: (Fx, Fx)) -> bool {
    let ground = field.height_at(spot.0, spot.1);
    // the camera sees over a flat stone, so those only count near the team
    let clear = |x: Fx, z: Fx, stones: bool| {
        polygon_contains(&map.outer, x, z)
            && (field.height_at(x, z) - ground).abs() < WIN_SLOPE
            && map.solids.list.iter().all(|s| {
                if !stones && s.part == Part::Slab {
                    return true;
                }
                let (nx, nz) = s.nearest(x, z);
                fx::length(nx - x, nz - z) > WIN_CLEAR
            })
    };
    let spaced = |from: Fx, to: Fx| {
        let steps = (to - from) / WIN_ROOM;
        (0..=steps).map(move |k| from + k * WIN_ROOM)
    };
    spaced(-WIN_BEHIND, WIN_PAST).all(|ahead| {
        spaced(-WIN_SIDE, WIN_SIDE).all(|side| {
            let (x, z) = ground_from(spot, ahead, side);
            clear(x, z, ahead > -WIN_STONES)
        })
    })
}

/// Where the lineup and a winning team stand: the nearest open ground to
/// `SPOT`, looking the same way. It comes from the map, so a wall moving
/// can't cut across them; with none in reach they stand on `SPOT`.
pub fn podium(map: &Map, field: &Field) -> (Fx, Fx) {
    (0..WIN_RINGS)
        .flat_map(|ring| {
            let across = move || -ring..=ring;
            across().flat_map(move |a| {
                across()
                    .filter(move |s| a.abs().max(s.abs()) == ring)
                    .map(move |s| ground_at(a * WIN_STEP, s * WIN_STEP))
            })
        })
        .find(|spot| open_at(map, field, *spot))
        .unwrap_or(SPOT)
}

/// The model a seat shows: a role not yet drawn stands as a ewe.
fn model_of(look: &str) -> Model {
    match look {
        "sheepdog" => Model::Sheepdog,
        "wolf" => Model::Wolf,
        _ => Model::Ewe,
    }
}

/// Everyone seated, stood on the ground before the camera.
fn stands_on(seats: &[LineupSeat], spot: (Fx, Fx), field: &Field) -> Vec<Stand> {
    let models: Vec<Model> = seats.iter().map(|s| model_of(&s.look)).collect();
    let yours = seats.iter().position(|s| s.me).unwrap_or(seats.len());
    let mut stands = lineup(&models, yours);
    for s in &mut stands {
        let toward_camera = if s.yours { s.at.z } else { mul(s.at.z, LINEUP_DEEP) };
        let side = if !s.yours && s.at.x.abs() < cm(30) {
            s.at.x + LINEUP_ASIDE
        } else {
            s.at.x
        };
        let (x, z) = ground_from(spot, -toward_camera, mul(side, LINEUP_TIGHT));
        s.at = v3(x, field.height_at(x, z), z);
    }
    stands
}

/// A winning team, stood on the ground before the camera.
fn winners_on(seats: &[LineupSeat], spot: (Fx, Fx), field: &Field) -> Vec<Stand> {
    let models: Vec<Model> = seats.iter().map(|s| model_of(&s.look)).collect();
    let mut stands = victors(&models);
    for s in &mut stands {
        let (x, z) = ground_from(spot, -s.at.z, s.at.x);
        s.at = v3(x, field.height_at(x, z), z);
    }
    stands
}

/// How far from the middle of the line the outermost character stands.
fn reach_of(stands: &[Stand], spot: (Fx, Fx)) -> Fx {
    let (dx, dz) = along();
    let side = |s: &Stand| mul(s.at.x - spot.0, dz) - mul(s.at.z - spot.1, dx);
    stands.iter().map(|s| side(s).abs()).max().unwrap_or(0)
}

fn tan(angle: i32) -> Fx {
    fx::div(fx::sin(angle), fx::cos(angle))
}

/// How much of the window's width and height is open for the lineup, in CSS
/// pixels, and how far the open part's middle sits above the window's, as a
/// share of half the height.
fn open_of(stage: &Stage) -> (i32, i32, Fx) {
    match stage.dock {
        Dock::Sides { open_w, .. } => (open_w as i32, stage.height, 0),
        Dock::Below { top } => (
            stage.width,
            top as i32,
            ONE - fx::ratio(top as i32, stage.height.max(1)),
        ),
        Dock::Center => (stage.width, stage.height, 0),
    }
}

/// How far back the lineup's camera stands, so the outermost character is in
/// the open part of the window.
fn lobby_distance(stage: &Stage, reach: Fx) -> Fx {
    let (open_w, _, _) = open_of(stage);
    let half = mul(tan(LOBBY_FOV / 2), fx::ratio(open_w.max(1), stage.height.max(1)));
    fx::div(reach + LINEUP_MARGIN, half.max(1)).clamp(LOBBY_NEAREST, LOBBY_FARTHEST)
}

/// The winners' screen has one panel, on the left: how much of the window's
/// width is open past it, and how far right of the window's middle that
/// part's middle sits, as a share of half the width.
fn win_open(stage: &Stage) -> (i32, Fx) {
    match stage.dock {
        Dock::Sides { open_x, .. } => (
            stage.width - open_x as i32,
            fx::ratio(open_x as i32, stage.width.max(1)),
        ),
        _ => (open_of(stage).0, 0),
    }
}

impl Backdrop {
    pub fn new(field: &Field) -> Backdrop {
        let stage = Stage {
            width: 1920,
            height: 1080,
            dock: Dock::Center,
            lineup: Vec::new(),
            cheer: false,
        };
        let mut b = Backdrop {
            stage,
            podium: SPOT,
            stands: Vec::new(),
            shot: Shot {
                eye: V3::default(),
                aim: V3::default(),
                fov: TITLE_FOV,
            },
            clock: 0,
            spin: 0,
            still: false,
            next_frame: 0,
        };
        b.shot = b.wanted(field);
        b
    }

    /// Where a winning team stands, from the map.
    pub fn set_podium(&mut self, spot: (Fx, Fx)) {
        self.podium = spot;
    }

    pub fn stands(&self) -> &[Stand] {
        &self.stands
    }

    pub fn spin(&self) -> u32 {
        self.spin
    }

    /// The way a winning team faces, when this is the winners' screen.
    pub fn cheering(&self) -> Option<i32> {
        self.stage.cheer.then_some(HEADING + TURN / 2)
    }

    /// The clock a winning team moves by: none in a still scene.
    pub fn win_clock(&self) -> Option<u32> {
        (!self.still).then_some(self.clock)
    }

    /// The page's word on what is on screen, as JSON; anything else is
    /// ignored and the scene stands as it was.
    pub fn set_stage(&mut self, text: &str, field: &Field) {
        if let Ok(stage) = serde_json::from_str::<Stage>(text) {
            self.stands = if stage.cheer {
                winners_on(&stage.lineup, self.podium, field)
            } else {
                stands_on(&stage.lineup, self.podium, field)
            };
            let cut = stage.cheer != self.stage.cheer;
            self.stage = stage;
            if self.still || cut {
                self.shot = self.wanted(field);
            }
        }
    }

    /// Reduced motion: the camera goes straight to its shot and stays.
    pub fn set_still(&mut self, still: bool, field: &Field) {
        self.still = still;
        self.clock = 0;
        self.shot = self.wanted(field);
    }

    /// The sway's two waves, -1 to 1.
    fn waves(&self) -> (Fx, Fx) {
        let at = (self.clock % SWAY_TICKS) as i64 * TURN as i64 / SWAY_TICKS as i64;
        (fx::sin(at as i32), fx::sin((at * 2) as i32))
    }

    fn title_shot(&self, field: &Field) -> Shot {
        let (sway, breath) = self.waves();
        let (x, z) = ground_at(mul(breath, TITLE_BREATH) - TITLE_BACK, mul(sway, TITLE_SWAY));
        let (ax, az) = ground_at(TITLE_AIM, 0);
        let eye_y = field.height_at(SPOT.0, SPOT.1) + TITLE_UP;
        Shot {
            eye: v3(x, eye_y, z),
            aim: v3(ax, eye_y + TITLE_AIM_UP, az),
            fov: TITLE_FOV,
        }
    }

    fn lobby_shot(&self, field: &Field) -> Shot {
        let (sway, _) = self.waves();
        let spot = self.podium;
        let back = lobby_distance(&self.stage, reach_of(&self.stands, spot));
        let (x, z) = ground_from(spot, -back, mul(sway, LOBBY_SWAY));
        let ground = field.height_at(spot.0, spot.1);
        let (_, _, raised) = open_of(&self.stage);
        let lower = mul(mul(back, tan(LOBBY_FOV / 2)), raised);
        Shot {
            eye: v3(x, ground + LOBBY_UP, z),
            aim: v3(spot.0, ground + LOBBY_AIM_UP - lower, spot.1),
            fov: LOBBY_FOV,
        }
    }

    /// Low in the grass before the team, the sky behind their heads.
    fn win_shot(&self, field: &Field) -> Shot {
        let (sway, _) = self.waves();
        let spot = self.podium;
        let (free, right) = win_open(&self.stage);
        let height = self.stage.height.max(1);
        let half = mul(tan(LOBBY_FOV / 2), fx::ratio(free.max(1), height));
        let reach = reach_of(&self.stands, spot) + WIN_MARGIN;
        let back = fx::div(reach, half.max(1)).clamp(WIN_NEAREST, LOBBY_FARTHEST);
        let (x, z) = ground_from(spot, -back, mul(sway, WIN_SWAY));
        let ground = field.height_at(spot.0, spot.1);
        let (_, _, raised) = open_of(&self.stage);
        let tall = mul(back, tan(LOBBY_FOV / 2));
        let lower = mul(tall, raised);
        let across = mul(mul(tall, fx::ratio(self.stage.width, height)), right);
        let (ax, az) = ground_from(spot, 0, -across);
        Shot {
            eye: v3(x, field.height_at(x, z) + WIN_UP, z),
            aim: v3(ax, ground + WIN_AIM_UP - lower, az),
            fov: LOBBY_FOV,
        }
    }

    fn wanted(&self, field: &Field) -> Shot {
        if self.stands.is_empty() {
            self.title_shot(field)
        } else if self.stage.cheer {
            self.win_shot(field)
        } else {
            self.lobby_shot(field)
        }
    }

    pub fn advance(&mut self, steps: u32, field: &Field) {
        self.spin = self.spin.wrapping_add(steps);
        if self.still {
            return;
        }
        for _ in 0..steps {
            self.clock = self.clock.wrapping_add(1);
            let to = self.wanted(field);
            let ease = |a: Fx, b: Fx| a + ((b - a) >> EASE_SHIFT);
            let near = |a: V3, b: V3| v3(ease(a.x, b.x), ease(a.y, b.y), ease(a.z, b.z));
            self.shot = Shot {
                eye: near(self.shot.eye, to.eye),
                aim: near(self.shot.aim, to.aim),
                fov: ease(self.shot.fov, to.fov),
            };
        }
    }

    pub fn camera(&self) -> Camera {
        Camera {
            eye: self.shot.eye,
            fwd: self.shot.aim.sub(self.shot.eye).norm(),
            fov: self.shot.fov,
            near: NEAR,
            far: FAR,
        }
    }

    /// Is a frame due at this time? The cap holds on any screen's rate, and
    /// a late frame doesn't bring on a burst to catch up.
    pub fn due(&mut self, now_us: i64) -> bool {
        if now_us < self.next_frame {
            return false;
        }
        let behind = now_us - self.next_frame >= FRAME_US;
        self.next_frame = FRAME_US + if behind { now_us } else { self.next_frame };
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::geom::Mat4;
    use crate::scape::view::Hillside;

    fn seat(n: u32, look: &str, me: bool) -> LineupSeat {
        LineupSeat {
            seat: n,
            look: look.into(),
            name: String::new(),
            me,
        }
    }

    fn stage(seats: Vec<LineupSeat>, width: i32, height: i32, dock: Dock) -> String {
        let s = serde_json::json!({ "width": width, "height": height, "dock": dock, "lineup": seats });
        s.to_string()
    }

    fn eight() -> Vec<LineupSeat> {
        (0..8)
            .map(|n| seat(n, if n < 4 { "sheepdog" } else { "wolf" }, n == 5))
            .collect()
    }

    /// Screen x and y of a point, -1 to 1 across the window.
    fn on_screen(cam: &Camera, width: i32, height: i32, p: V3) -> (Fx, Fx) {
        let view = Mat4::look(cam.eye, cam.fwd, V3::UP);
        let proj = Mat4::perspective(cam.fov, fx::ratio(width, height), cam.near, cam.far);
        let [x, y, _, w] = proj.mul(&view).project(p);
        (fx::div(x, w), fx::div(y, w))
    }

    #[test]
    fn the_lineup_stands_clear_of_both_docked_panels_and_centered_between_them() {
        for (width, height) in [(1920, 1080), (1366, 768), (2560, 1080)] {
            let mut h = Hillside::backdrop();
            let dock = crate::dock::lobby(width as u32, height as u32);
            h.set_still(true);
            h.set_stage(&stage(eight(), width, height, dock));
            let scene = h.scene().unwrap();
            let cam = scene.camera();
            let Dock::Sides { open_x, open_w, .. } = dock else {
                panic!("expected panels at the sides")
            };
            let left = 2 * open_x as i32 - width;
            let right = 2 * (open_x + open_w) as i32 - width;
            let mut xs: Vec<Fx> = Vec::new();
            // a character's length to either side of its feet
            let (dx, dz) = along();
            let side = v3(mul(-dz, cm(130)), 0, mul(dx, cm(130)));
            for s in scene.stands() {
                xs.push(on_screen(&cam, width, height, s.at.add(side)).0);
                xs.push(on_screen(&cam, width, height, s.at.sub(side)).0);
            }
            let (lo, hi) = (*xs.iter().min().unwrap(), *xs.iter().max().unwrap());
            assert!(lo > fx::ratio(left, width), "{width}: {lo} past the left panel");
            assert!(hi < fx::ratio(right, width), "{width}: {hi} past the right panel");
            assert!((lo + hi).abs() < cm(4), "{width}: off center by {}", lo + hi);
        }
    }

    #[test]
    fn with_the_panels_below_the_lineup_stands_in_the_top_part() {
        let mut h = Hillside::backdrop();
        h.set_still(true);
        h.set_stage(&stage(eight(), 1000, 800, crate::dock::lobby(1000, 800)));
        let scene = h.scene().unwrap();
        let cam = scene.camera();
        for s in scene.stands() {
            let (_, y) = on_screen(&cam, 1000, 800, s.at);
            assert!(y > cm(5), "{y} is under the panels");
        }
    }

    #[test]
    fn the_lineup_stands_on_the_same_open_ground_as_the_winners() {
        let mut h = Hillside::backdrop();
        h.set_still(true);
        h.set_stage(&stage(eight(), 1920, 1080, crate::dock::lobby(1920, 1080)));
        let scene = h.scene().unwrap();
        let spot = scene.podium;
        let map = crate::map::Map::generate(SEED as u64);
        assert!(open_at(&map, &h.scape().field, spot));
        let you = scene.stands().iter().find(|s| s.yours).unwrap();
        // the plinth stands 2.3 m before the spot, toward the camera
        assert!((ahead_of(spot, you.at.x, you.at.z) + cm(230)).abs() < cm(1));
    }

    #[test]
    fn your_own_character_stands_on_the_plinth_nearest_the_camera() {
        let mut h = Hillside::backdrop();
        h.set_stage(&stage(eight(), 1920, 1080, crate::dock::lobby(1920, 1080)));
        let scene = h.scene().unwrap();
        let (you, rest): (Vec<&Stand>, Vec<&Stand>) = scene.stands().iter().partition(|s| s.yours);
        assert_eq!(you.len(), 1);
        assert_eq!(you[0].model, Model::Wolf);
        let eye = scene.camera().eye;
        assert!(rest.iter().all(|s| s.at.dist(eye) > you[0].at.dist(eye)));
    }

    #[test]
    fn a_sixty_hertz_screen_draws_every_other_frame_and_a_stall_brings_no_burst() {
        let scene = Hillside::backdrop();
        let mut b = Backdrop::new(&scene.scape().field);
        // one second at 60 frames, a little over the cap's own period each
        let drawn = (0..60).filter(|k| b.due(k * 16_667)).count();
        assert_eq!(drawn, 30);
        assert!(b.due(9_000_000));
        assert!(!b.due(9_016_667));
    }

    #[test]
    fn the_lineup_spins_under_reduced_motion_and_the_camera_holds() {
        let hill = Hillside::backdrop();
        let field = &hill.scape().field;
        let mut scene = Backdrop::new(field);
        scene.set_still(true, field);
        let shot = scene.shot;
        scene.advance(3, field);
        assert_eq!(scene.spin(), 3);
        assert_eq!(scene.shot, shot);
    }

    #[test]
    fn nobody_stands_straight_behind_your_own_character() {
        let seats = vec![seat(0, "sheepdog", true), seat(1, "wolf", false)];
        let mut h = Hillside::backdrop();
        h.set_stage(&stage(seats, 1920, 1080, crate::dock::lobby(1920, 1080)));
        let scene = h.scene().unwrap();
        let (you, other) = (&scene.stands()[1], &scene.stands()[0]);
        assert!(you.yours);
        let (dx, dz) = along();
        let across = mul(other.at.x - you.at.x, dz) - mul(other.at.z - you.at.z, dx);
        assert!(across.abs() > cm(55), "{across}");
    }

    fn winners_stage(n: u32, width: i32, height: i32) -> String {
        let seats: Vec<LineupSeat> = (0..n).map(|k| seat(k, "wolf", k == 0)).collect();
        let s = serde_json::json!({
            "width": width, "height": height, "lineup": seats, "cheer": true,
            "dock": crate::dock::lobby(width as u32, height as u32),
        });
        s.to_string()
    }

    #[test]
    fn nothing_solid_stands_between_the_camera_and_the_winning_team() {
        let mut h = Hillside::backdrop();
        h.set_still(true);
        h.set_stage(&winners_stage(7, 1920, 1080));
        let scene = h.scene().unwrap();
        let eye = scene.camera().eye;
        assert_eq!(scene.stands().len(), 7);
        let map = crate::map::Map::generate(SEED as u64);
        for s in scene.stands() {
            for k in 0..=24 {
                let (x, z) = (
                    eye.x + (s.at.x - eye.x) / 24 * k,
                    eye.z + (s.at.z - eye.z) / 24 * k,
                );
                for solid in &map.solids.list {
                    // the eye stands 1.3 m up, over a flat stone
                    if solid.part == Part::Slab && k < 20 {
                        continue;
                    }
                    let (nx, nz) = solid.nearest(x, z);
                    assert!(
                        fx::length(nx - x, nz - z) > cm(60),
                        "{k} of the way to {:?}",
                        s.at
                    );
                }
            }
        }
    }

    #[test]
    fn the_winners_camera_stands_above_the_grass_and_looks_at_them() {
        let mut h = Hillside::backdrop();
        h.set_still(true);
        h.set_stage(&winners_stage(7, 1920, 1080));
        let field = &h.scape().field;
        let scene = h.scene().unwrap();
        let cam = scene.camera();
        assert_eq!(cam.eye.y - field.height_at(cam.eye.x, cam.eye.z), WIN_UP);
        for s in scene.stands() {
            let (x, y) = on_screen(&cam, 1920, 1080, s.at);
            assert!(x.abs() < ONE && y.abs() < ONE, "{x}, {y} is off the window");
            // feet below the middle
            // right of the panel, whose edge is 448 of 1920 across
            assert!(x > fx::ratio(2 * 448 - 1920, 1920), "{x} is under the panel");
            assert!(y < 0, "{y} is above the middle of the window");
        }
    }

    #[test]
    fn a_winners_stage_is_a_cut_and_the_lobby_after_it_is_a_cut_back() {
        let mut h = Hillside::backdrop();
        h.set_stage(&stage(eight(), 1920, 1080, crate::dock::lobby(1920, 1080)));
        let spot = h.scene().unwrap().podium;
        h.set_stage(&winners_stage(3, 1920, 1080));
        let scene = h.scene().unwrap();
        assert!(scene.cheering().is_some());
        // three stand close: the camera comes in to its nearest
        let d = fx::length(scene.camera().eye.x - spot.0, scene.camera().eye.z - spot.1);
        assert!(d >= WIN_NEAREST - cm(1) && d < WIN_NEAREST + cm(1), "{d}");
        h.set_stage(&stage(eight(), 1920, 1080, crate::dock::lobby(1920, 1080)));
        assert!(h.scene().unwrap().cheering().is_none());
        assert_eq!(h.scene().unwrap().stands().len(), 8);
    }

    #[test]
    fn with_reduced_motion_a_winning_team_has_no_clock() {
        let mut h = Hillside::backdrop();
        h.set_stage(&winners_stage(3, 1920, 1080));
        assert_eq!(h.scene().unwrap().win_clock(), Some(0));
        h.set_still(true);
        assert_eq!(h.scene().unwrap().win_clock(), None);
    }

    #[test]
    fn a_seat_with_no_role_yet_stands_as_a_ewe() {
        assert_eq!(model_of("random"), Model::Ewe);
        assert_eq!(model_of("wolf"), Model::Wolf);
        assert_eq!(model_of("sheepdog"), Model::Sheepdog);
    }

    #[test]
    fn the_sway_is_slow_and_comes_back_and_a_still_scene_does_not_move() {
        let field_scene = Hillside::backdrop();
        let field = &field_scene.scape().field;
        let mut b = Backdrop::new(field);
        let start = b.camera().eye;
        let mut last = start;
        for _ in 0..SWAY_TICKS {
            b.advance(1, field);
            let eye = b.camera().eye;
            assert!(eye.dist(last) < cm(2), "a tick moved {}", eye.dist(last));
            last = eye;
        }
        assert!(last.dist(start) < cm(30), "{}", last.dist(start));
        b.set_still(true, field);
        let held = b.camera();
        b.advance(600, field);
        assert_eq!(b.camera().eye, held.eye);
    }

    #[test]
    fn a_stage_that_will_not_parse_changes_nothing() {
        let mut h = Hillside::backdrop();
        h.set_stage(&stage(eight(), 1920, 1080, crate::dock::lobby(1920, 1080)));
        h.set_stage("not json");
        assert_eq!(h.scene().unwrap().stands().len(), 8);
    }

    /// The fixed view is a place on a generated map: when the map changes,
    /// this says the camera or the lineup now stands in something.
    #[test]
    fn the_camera_and_the_lineup_stand_clear_of_trees_and_rocks() {
        let map = crate::map::Map::generate(SEED as u64);
        let clear = |p: (Fx, Fx), room: Fx| {
            map.trees.iter().all(|t| fx::length(t.x - p.0, t.z - p.1) > room)
                && map.cairns.iter().all(|c| fx::length(c.x - p.0, c.z - p.1) > room)
                && map.mounds.iter().all(|m| fx::length(m.x - p.0, m.z - p.1) > room)
        };
        assert!(clear(SPOT, from_int(9)));
        assert!(clear(ground_at(-TITLE_BACK, 0), from_int(8)));
    }
}
