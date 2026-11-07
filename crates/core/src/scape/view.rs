//! The hillside on its own (`web/hillside.html`): the map's world with a
//! flock on it, built from the page's query string, so a screenshot is a link.
//!
//! `?seed=3&sheep=100&phase=0&x=120&z=60&yaw=30&back=5&up=3&tick=90`
//! `phase` is a percent of the way to night; `howl=1` runs the change there
//! and back. `x`, `z` and `yaw` (degrees) put the Sheepdog; the camera is
//! `back` meters behind and `up` meters above, as in play (both in
//! centimeters). `pair=1` crouches a Wolf and stands a lamb in the view, to
//! check the grass doesn't hide them.

use serde_json::json;

use crate::fx::{self, from_int, mul, Fx, ONE};
use crate::map::{Map, MapSize};
use crate::models::rig::{Clip, Walker};
use crate::models::sheep::{coat_of, size_of};
use crate::models::{character_coat, cheering, push_animal, push_shadow, turntable, Animal, Library, Model};
use crate::render::geom::{cm, deg, v3, V3};
use crate::render::{buffer_size, view_values, Camera, Frame, BYTES_PER_VALUE, DRAW_VALUES};

use super::backdrop::{self, Backdrop};
use super::grass::{
    self, Bake, BEND_FULL, MOTTLE_BAND, MOTTLE_FAR, MOTTLE_HIGH, MOTTLE_LOW, MOTTLE_MIX, MOTTLE_SHIFT_XZ,
    MOTTLE_SIZES, ORIGIN, PRISM_INDICES, STEP_SHIFT_XZ, STEP_SHIFT_Y, TIERS, VERTEX_BYTES,
};
use super::hills::REACH;
use super::noise::{MOTTLE_HASH, MOTTLE_SHIFT, MOTTLE_TOP};
use super::shapes::{kind_of, Kind};
use super::sky;
use super::wind::Wind;
use super::world::Scape;

/// A blade darkens toward its foot by this much at the near edge.
const NEAR_DARK: Fx = ONE * 95 / 100;
const NEAR_DARK_AT: Fx = from_int(6);
const NEAR_DARK_GONE: Fx = from_int(18);
/// The ground's own shading, from `notes/grass.md`.
const SHEEN: Fx = ONE / 10;
const SHADOW: Fx = ONE * 8 / 100;
const TROUGH: Fx = ONE * 12 / 100;
/// Two crossing waves, radians a meter: 2π / 3.3 m and 2π / 5.1 m.
const RIDGES: [Fx; 4] = [100_000, 57_000, -38_000, 65_000];
/// The uniforms of the grass values, in order: name, size, call.
const UNIFORMS: [(&str, usize, &str); 22] = [
    ("uOrigin", 3, "uniform3fv"),
    ("uStep", 3, "uniform3fv"),
    ("uDir", 2, "uniform2fv"),
    ("uBend", 1, "uniform1fv"),
    ("uKx", 4, "uniform4fv"),
    ("uKz", 4, "uniform4fv"),
    ("uPhase", 4, "uniform4fv"),
    ("uShare", 4, "uniform4fv"),
    ("uNear", 3, "uniform3fv"),
    ("uTall", 1, "uniform1fv"),
    ("uRidge", 4, "uniform4fv"),
    ("uGroundShade", 3, "uniform3fv"),
    ("uHalf", 1, "uniform1fv"),
    ("uMottleHash", 4, "uniform4fv"),
    ("uMottleTop", 1, "uniform1fv"),
    ("uMottleMix", 3, "uniform3fv"),
    ("uMottleShift", 2, "uniform2fv"),
    ("uMottleBand", 2, "uniform2fv"),
    ("uMottleSizes", 3, "uniform3fv"),
    ("uMottleFar", 1, "uniform1fv"),
    ("uMottleLow", 3, "uniform3fv"),
    ("uMottleHigh", 3, "uniform3fv"),
];

/// One flock member.
#[derive(Debug, Clone, Copy)]
struct Ewe {
    walker: Walker,
    at: V3,
    yaw: i32,
    up: V3,
    seed: i32,
}

struct Query(Vec<(String, String)>);

impl Query {
    fn int(&self, key: &str, default: i32) -> i32 {
        let found = self.0.iter().find(|(k, _)| k == key);
        found.and_then(|(_, v)| v.parse().ok()).unwrap_or(default)
    }
}

fn parse(query: &str) -> Query {
    let pairs = query
        .trim_start_matches('?')
        .split('&')
        .filter_map(|p| p.split_once('='));
    Query(pairs.map(|(k, v)| (k.to_string(), v.to_string())).collect())
}

/// Clearance from a tree's crown to the eye: a sycamore's lobes droop to about
/// 3 m off the ground, and one this close fills a side of the view (R1.716).
const CROWN_CLEAR: Fx = from_int(12);
const STAND_BACK: Fx = cm(500);

/// How far the eye of a stand at (x, z), facing `yaw`, is outside every crown.
fn crown_room(map: &Map, x: Fx, z: Fx, yaw: i32) -> Fx {
    let (ex, ez) = (
        x - mul(fx::sin(yaw), STAND_BACK),
        z - mul(fx::cos(yaw), STAND_BACK),
    );
    let room = map.trees.iter().map(|t| fx::length(t.x - ex, t.z - ez) - t.crown);
    room.min().unwrap_or(from_int(100))
}

/// The first spot and heading worth a look: the Sheepdogs' corner, facing the
/// middle of the hill, walked on toward the middle until the eye is clear of
/// every tree.
fn default_stand(map: &Map) -> (Fx, Fx, i32) {
    let (x, z) = map
        .sheepdog_starts
        .first()
        .copied()
        .unwrap_or((from_int(30), from_int(30)));
    let (mx, mz) = (map.length() / 2, map.width() / 2);
    let yaw = crate::render::geom::atan2(mx - x, mz - z);
    for step in 0..30 {
        let on = from_int(step * 3);
        let (sx, sz) = (x + mul(fx::sin(yaw), on), z + mul(fx::cos(yaw), on));
        if crown_room(map, sx, sz, yaw) >= CROWN_CLEAR {
            return (sx, sz, yaw);
        }
    }
    (x, z, yaw)
}

fn tree_of(map: &Map, kind: Kind) -> Option<(Fx, Fx)> {
    let found = map.trees.iter().find(|t| kind_of(t) == kind);
    found.map(|t| (t.x, t.z))
}

/// The named things worth a close look.
fn spot(map: &Map, name: &str) -> Option<(Fx, Fx)> {
    match name {
        "fold" => Some(map.fold.gap.center),
        "gap" => map.gaps.first().map(|g| g.center),
        "mouth" => map.mouths().first().copied(),
        "cairn" => map.cairns.first().map(|c| (c.x, c.z)),
        "mound" => map.mounds.first().map(|m| (m.x, m.z)),
        "dug" => map.mounds.get(3).map(|m| (m.x, m.z)),
        "tree" => map.trees.first().map(|t| (t.x, t.z)),
        "slab" => map.slabs.first().map(|s| s.center()),
        "pine" => tree_of(map, Kind::Pine),
        "rowan" => tree_of(map, Kind::Rowan),
        _ => None,
    }
}

/// A stand `dist` meters from `p` on the middle of the map's side, facing it.
fn stand_near(map: &Map, p: (Fx, Fx), dist: Fx) -> (Fx, Fx, i32) {
    let (mx, mz) = (map.length() / 2, map.width() / 2);
    let (dx, dz) = (mx - p.0, mz - p.1);
    let len = fx::length(dx, dz).max(ONE);
    let (ux, uz) = (fx::div(dx, len), fx::div(dz, len));
    (
        p.0 + mul(ux, dist),
        p.1 + mul(uz, dist),
        crate::render::geom::atan2(-ux, -uz),
    )
}

/// A Wolf or lamb set in the view by `pair=1`.
#[derive(Debug, Clone, Copy)]
struct Guest {
    model: Model,
    walker: Walker,
    at: V3,
    yaw: i32,
}

pub struct Hillside {
    lib: Library,
    map: Map,
    seed: u32,
    scape: Scape,
    grass: Bake,
    flock: Vec<Ewe>,
    guests: Vec<Guest>,
    size: [i32; 2],
    at: (Fx, Fx),
    yaw: i32,
    back: Fx,
    up: Fx,
    phase: Fx,
    /// how much of the light a viewer has, as a team's night
    light: Fx,
    howl: bool,
    still: bool,
    tick: u32,
    /// behind the menus: its own camera, and the lobby's lineup
    backdrop: Option<Backdrop>,
    /// A match's camera, once `set_view` has run.
    play: Option<Camera>,
}

impl Hillside {
    pub fn new(query: &str) -> Hillside {
        let q = parse(query);
        let seed = q.int("seed", 3) as u64;
        let sizes = MapSize::ALL;
        let size = sizes[(q.int("size", 2).clamp(0, 2)) as usize];
        let map = Map::sized(seed, size);
        let scape = Scape::build(&map, seed as u32);
        let lib = Library::build();
        let n = q.int("sheep", 100).max(0) as usize;
        let flock = map
            .sheep_starts(n)
            .iter()
            .enumerate()
            .map(|(k, p)| {
                let i = k as i32;
                let [nx, ny, nz] = scape.field.normal_near(p.0, p.1);
                Ewe {
                    walker: Walker::new(if i % 3 == 0 { Clip::Idle } else { Clip::Graze }, (i * 41) % ONE),
                    at: v3(p.0, scape.field.height_at(p.0, p.1), p.1),
                    yaw: deg((i * 137) % 360),
                    up: v3(nx, ny, nz),
                    seed: i,
                }
            })
            .collect();
        let named =
            q.0.iter()
                .find(|(k, _)| k == "look")
                .and_then(|(_, v)| spot(&map, v));
        let close = fx::ratio(q.int("dist", 1200), 100);
        let (x, z, yaw) = named.map_or(default_stand(&map), |p| stand_near(&map, p, close));
        let has = |key: &str| q.int(key, i32::MIN) != i32::MIN;
        let at = (
            if has("x") { from_int(q.int("x", 0)) } else { x },
            if has("z") { from_int(q.int("z", 0)) } else { z },
        );
        let yaw = if has("yaw") { deg(q.int("yaw", 0)) } else { yaw };
        let guest = |model, clip, ahead: Fx, aside: Fx| {
            let (dx, dz) = (fx::sin(yaw), fx::cos(yaw));
            let gx = at.0 + mul(dx, ahead) + mul(dz, aside);
            let gz = at.1 + mul(dz, ahead) - mul(dx, aside);
            Guest {
                model,
                walker: Walker::new(clip, 0),
                at: v3(gx, scape.field.height_at(gx, gz), gz),
                yaw: yaw + deg(100),
            }
        };
        let guests = if q.int("pair", 0) != 0 {
            vec![
                guest(Model::Wolf, Clip::Crouch, from_int(4), -from_int(2)),
                guest(Model::Lamb, Clip::Idle, from_int(4), from_int(2)),
            ]
        } else {
            Vec::new()
        };
        Hillside {
            at,
            yaw,
            back: fx::ratio(q.int("back", 500), 100),
            up: fx::ratio(q.int("up", 300), 100),
            phase: fx::ratio(q.int("phase", 0), 100),
            light: fx::ratio(q.int("light", 100), 100),
            howl: q.int("howl", 0) != 0,
            still: has("tick"),
            tick: q.int("tick", 0) as u32,
            play: None,
            size: [1920, 1080],
            lib,
            seed: seed as u32,
            scape,
            backdrop: None,
            grass: Bake {
                vertices: Vec::new(),
                chunks: Vec::new(),
            },
            flock,
            guests,
            map,
        }
    }

    /// The scene behind the menus: a fixed seed and sunset, and the sheep
    /// nearest one spot in the middle distance (`backdrop.rs`).
    pub fn backdrop() -> Hillside {
        let query = format!(
            "?seed={}&sheep={}&phase={}",
            backdrop::SEED,
            backdrop::SCATTERED,
            backdrop::PHASE
        );
        let mut h = Hillside::new(&query);
        let podium = backdrop::podium(&h.map, &h.scape.field);
        let (x, z) = backdrop::flock_at(podium);
        h.flock
            .retain(|e| backdrop::ahead_of(podium, e.at.x, e.at.z) > backdrop::FLOCK_PAST);
        h.flock.sort_by_key(|e| fx::length(e.at.x - x, e.at.z - z));
        h.flock.truncate(backdrop::FLOCK);
        let mut scene = Backdrop::new(&h.scape.field);
        scene.set_podium(podium);
        h.backdrop = Some(scene);
        h
    }

    /// What is on screen, as the page's JSON. Only the backdrop reads it.
    pub fn set_stage(&mut self, text: &str) {
        if let Some(b) = &mut self.backdrop {
            b.set_stage(text, &self.scape.field);
        }
    }

    /// Is a frame due? Only the backdrop keeps a cap.
    pub fn due(&mut self, now_us: i64) -> bool {
        self.backdrop.as_mut().is_none_or(|b| b.due(now_us))
    }

    pub fn set_still(&mut self, still: bool) {
        if let Some(b) = &mut self.backdrop {
            b.set_still(still, &self.scape.field);
        }
    }

    pub fn scene(&self) -> Option<&Backdrop> {
        self.backdrop.as_ref()
    }

    /// The hillside of a match: no flock of its own, and a camera from
    /// `set_view`.
    pub fn of_match(map: Map, seed: u64) -> Hillside {
        Hillside {
            at: (0, 0),
            yaw: 0,
            back: 0,
            up: 0,
            phase: 0,
            light: ONE,
            howl: false,
            still: true,
            tick: 0,
            play: None,
            size: [1920, 1080],
            lib: Library::build(),
            seed: seed as u32,
            scape: Scape::build(&map, seed as u32),
            grass: Bake {
                vertices: Vec::new(),
                chunks: Vec::new(),
            },
            flock: Vec::new(),
            guests: Vec::new(),
            backdrop: None,
            map,
        }
    }

    /// The camera and the tick the grass reads this frame.
    pub fn set_view(&mut self, cam: Camera, tick: u32) {
        self.play = Some(cam);
        self.tick = tick;
    }

    pub fn size(&self) -> [i32; 2] {
        self.size
    }

    pub fn library(&self) -> &Library {
        &self.lib
    }

    pub fn scape(&self) -> &Scape {
        &self.scape
    }

    pub fn grass(&self) -> &Bake {
        &self.grass
    }

    /// Once a match: its own call, so its cost can be read on its own.
    pub fn bake(&mut self) {
        self.grass = self.scape.bake_grass(&self.map, self.seed);
    }

    pub fn grass_indices(&self) -> Vec<u16> {
        grass::shared_indices(grass::most_prisms(&self.grass), grass::most_spikes(&self.grass))
    }

    /// Every number the page needs: the renderer's, and the grass's.
    pub fn numbers(&self) -> String {
        let base = self.lib.numbers();
        let mut n: serde_json::Value = serde_json::from_str(&base).unwrap_or(json!({}));
        let mut at = 0;
        let uniforms: Vec<serde_json::Value> = UNIFORMS
            .iter()
            .map(|(name, size, call)| {
                let row = json!([name, size, at, call]);
                at += size;
                row
            })
            .collect();
        let tiers: Vec<[usize; 4]> = (0..TIERS.len())
            .map(|t| [at + t * 5, at + t * 5 + 4, at + t * 5 + 4, at + t * 5 + 5])
            .collect();
        n["grass"] = json!({
            "stride": VERTEX_BYTES,
            "index_bytes": 2,
            "attributes": [
                ["aPos", 4, "UNSIGNED_SHORT", true, 0],
                ["aColor", 4, "UNSIGNED_BYTE", true, 8],
                ["aNormal", 4, "BYTE", true, 12],
            ],
            "ground": [["aPos", 3, "FLOAT", false, 0]],
            "draw": grass::DRAW_VALUES,
            "uniforms": uniforms,
            "tiers": tiers,
        });
        let _ = (DRAW_VALUES, BYTES_PER_VALUE, PRISM_INDICES);
        n.to_string()
    }

    /// The grass and ground uniforms, laid out by [`UNIFORMS`] and then, for
    /// each tier, its fade (four) and how far it sinks (one).
    pub fn grass_values(&self) -> Vec<i32> {
        let w: &Wind = &self.scape.wind;
        let mut v = Vec::new();
        v.extend_from_slice(&ORIGIN);
        // what a full 16-bit position spans, since it arrives as 0 to 1
        let (xz, y) = (65535 << STEP_SHIFT_XZ, 65535 << STEP_SHIFT_Y);
        v.extend_from_slice(&[xz, y, xz]);
        v.extend_from_slice(&w.dir);
        v.push(w.bend);
        v.extend_from_slice(&w.kx);
        v.extend_from_slice(&w.kz);
        v.extend_from_slice(&w.phases(self.tick));
        v.extend_from_slice(&w.share);
        v.extend_from_slice(&[NEAR_DARK_AT, NEAR_DARK_GONE, NEAR_DARK]);
        v.push(BEND_FULL);
        v.extend_from_slice(&RIDGES);
        v.extend_from_slice(&[SHEEN, SHADOW, TROUGH]);
        v.push(ONE / 2);
        v.extend(MOTTLE_HASH.map(|k| from_int(k as i32)));
        v.push(from_int(MOTTLE_SHIFT as i32));
        v.push(from_int(MOTTLE_TOP));
        v.extend_from_slice(&MOTTLE_MIX);
        v.extend_from_slice(&[MOTTLE_SHIFT_XZ.0, MOTTLE_SHIFT_XZ.1]);
        v.extend_from_slice(&MOTTLE_BAND);
        v.extend_from_slice(&MOTTLE_SIZES);
        v.push(MOTTLE_FAR);
        v.extend_from_slice(&MOTTLE_LOW);
        v.extend_from_slice(&MOTTLE_HIGH);
        for t in &TIERS {
            v.extend_from_slice(&t.fade);
            v.push(t.tall.1 + cm(6));
        }
        v
    }

    /// The ground's corners as x, y, z, for the ground's own shading.
    pub fn ground_points(&self) -> Vec<i32> {
        let f = &self.scape.field;
        let mut out = Vec::with_capacity(f.cols * f.rows * 3);
        for r in 0..f.rows {
            for c in 0..f.cols {
                out.extend_from_slice(&[
                    f.x0 + ((c as Fx) << 17),
                    f.heights[r * f.cols + c],
                    f.z0 + ((r as Fx) << 17),
                ]);
            }
        }
        out
    }

    pub fn resize(&mut self, css_w: i32, css_h: i32, dpr_pct: i32) -> [i32; 2] {
        let density = if self.backdrop.is_some() {
            dpr_pct.min(backdrop::DENSITY_PCT)
        } else {
            dpr_pct
        };
        self.size = buffer_size(css_w, css_h, density);
        self.size
    }

    pub fn advance(&mut self, now_us: i64) {
        let tick = (now_us / (1_000_000 / super::TICKS)) as u32;
        let steps = tick.wrapping_sub(self.tick).min(4);
        self.tick = tick;
        if let Some(b) = &mut self.backdrop {
            b.advance(steps, &self.scape.field);
        }
        if self.still {
            return;
        }
        for e in &mut self.flock {
            let clip = if e.seed % 3 == 0 { Clip::Idle } else { Clip::Graze };
            for _ in 0..steps {
                e.walker.step(Model::Ewe.rig(), clip, 0);
            }
        }
        if self.howl {
            // three seconds down, three up, then three of each held
            let t = (self.tick % 720) as Fx;
            let down = fx::ratio(t.min(180), 180);
            let up = fx::ratio((t - 360).clamp(0, 180), 180);
            self.phase = down - up;
        }
    }

    pub fn camera(&self) -> Camera {
        if let Some(b) = &self.backdrop {
            return b.camera();
        }
        if let Some(cam) = self.play {
            return cam;
        }
        let (x, z) = self.at;
        let (dx, dz) = (fx::sin(self.yaw), fx::cos(self.yaw));
        let dog = v3(x, self.scape.field.height_at(x, z), z);
        let (ex, ez) = (x - mul(dx, self.back), z - mul(dz, self.back));
        let clear = self.scape.field.height_at(ex, ez) + cm(40);
        let eye = v3(ex, (dog.y + self.up).max(clear), ez);
        let aim = dog.add(v3(mul(dx, from_int(14)), cm(60), mul(dz, from_int(14))));
        Camera {
            eye,
            fwd: aim.sub(eye).norm(),
            fov: deg(52),
            near: cm(10),
            far: REACH + from_int(300),
        }
    }

    pub fn frame(&self) -> Frame {
        let cam = self.camera();
        let sky = sky::dimmed(sky::at(self.phase), self.light);
        let mut b = self.lib.batch();
        for e in &self.flock {
            let size = size_of(e.seed);
            let coat = coat_of(e.seed);
            push_shadow(&mut b, Model::Ewe, e.at, e.yaw, e.up, size);
            push_animal(
                &mut b,
                cam.eye,
                &Animal {
                    model: Model::Ewe,
                    at: e.at,
                    yaw: e.yaw,
                    up: e.up,
                    size,
                    pose: e.walker.pose(Model::Ewe.rig(), &self.lib.poses),
                    coat: coat.wool,
                    skin: coat.skin,
                    outline: None,
                    bell: false,
                },
            );
        }
        for g in &self.guests {
            let (wool, skin) = match g.model {
                Model::Wolf => character_coat(Model::Wolf),
                _ => (coat_of(1).wool, coat_of(1).skin),
            };
            let [nx, ny, nz] = self.scape.field.normal_near(g.at.x, g.at.z);
            push_shadow(&mut b, g.model, g.at, g.yaw, v3(nx, ny, nz), ONE);
            push_animal(
                &mut b,
                cam.eye,
                &Animal {
                    model: g.model,
                    at: g.at,
                    yaw: g.yaw,
                    up: v3(nx, ny, nz),
                    size: ONE,
                    pose: g.walker.pose(g.model.rig(), &self.lib.poses),
                    coat: wool,
                    skin,
                    outline: None,
                    bell: false,
                },
            );
        }
        if let Some(scene) = &self.backdrop {
            let (stands, clock) = (scene.stands(), scene.spin());
            match scene.cheering() {
                Some(facing) => cheering(&mut b, &self.lib, stands, cam.eye, scene.win_clock(), facing),
                None => turntable(&mut b, &self.lib, stands, cam.eye, clock),
            }
        }
        let (mut instances, flock_rows, outlines) = b.finish(&self.lib.ranges);
        let (moving, mut moving_rows) = self.scape.moving(&|k| k % 3 == 0, self.tick);
        for row in moving_rows.chunks_mut(DRAW_VALUES) {
            row[5] += (instances.len() * BYTES_PER_VALUE) as u32;
        }
        instances.extend_from_slice(&moving);
        let mut draws = self.scape.rows_for(cam.eye, cam.fwd);
        draws.extend_from_slice(&moving_rows);
        draws.extend_from_slice(&flock_rows);
        Frame {
            view: view_values(&cam, self.size, &sky, false),
            instances,
            draws,
            outlines,
        }
    }

    pub fn grass_rows(&self) -> Vec<u32> {
        let cam = self.camera();
        grass::visible(
            &self.grass,
            [cam.eye.x, cam.eye.y, cam.eye.z],
            [cam.fwd.x, cam.fwd.y, cam.fwd.z],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_query_sets_the_stand_the_phase_and_the_flock() {
        let h = Hillside::new("?seed=2&sheep=12&phase=50&x=100&z=70&yaw=90&tick=30");
        assert_eq!(h.flock.len(), 12);
        assert_eq!(h.phase, ONE / 2);
        assert_eq!(h.at, (from_int(100), from_int(70)));
        assert!(h.still);
    }

    #[test]
    fn the_camera_stands_behind_and_above_the_sheepdog() {
        let h = Hillside::new("?seed=2&sheep=0&x=100&z=70&yaw=90");
        let cam = h.camera();
        let ground = h.scape.field.height_at(from_int(100), from_int(70));
        assert!(cam.eye.y - ground > cm(250));
        assert!(cam.fwd.x > cm(50));
        assert!(cam.eye.x < from_int(100));
    }

    /// R1.716: a stand at the Sheepdogs' corner could put the eye inside a
    /// sycamore's lobes, a dark block at the edge of every shot.
    #[test]
    fn the_default_stand_keeps_the_eye_clear_of_every_crown() {
        for seed in 1..=8 {
            let h = Hillside::new(&format!("?seed={seed}&sheep=0"));
            let eye = h.camera().eye;
            for t in &h.map.trees {
                let d = fx::length(t.x - eye.x, t.z - eye.z);
                assert!(
                    d - t.crown >= from_int(11),
                    "seed {seed}: {d} from a crown of {}",
                    t.crown
                );
            }
        }
    }

    #[test]
    fn a_frames_rows_all_fit_what_it_holds() {
        let h = Hillside::new("?seed=4&sheep=20");
        let f = h.frame();
        for row in f
            .draws
            .chunks(DRAW_VALUES)
            .filter(|r| r[4] == crate::render::FRAME_SOURCE)
        {
            let end = row[5] as usize + row[6] as usize * crate::render::INSTANCE_VALUES * BYTES_PER_VALUE;
            assert!(end <= f.instances.len() * BYTES_PER_VALUE);
        }
    }

    /// Three-part 8 and 16-bit attributes have no GPU format under Direct3D:
    /// the driver rewrites the whole buffer in software for every draw, 880
    /// ms a frame on the first measured run.
    #[test]
    fn every_packed_attribute_has_four_parts_and_is_normalized() {
        let h = Hillside::new("?seed=1&sheep=0");
        let n: serde_json::Value = serde_json::from_str(&h.numbers()).unwrap();
        for a in n["grass"]["attributes"].as_array().unwrap() {
            assert_eq!(a[1], 4, "{a}");
            assert_eq!(a[3], true, "{a}");
        }
    }

    #[test]
    fn the_uniform_table_ends_where_the_values_do() {
        let mut h = Hillside::new("?seed=1&sheep=0");
        h.bake();
        let total: usize = UNIFORMS.iter().map(|u| u.1).sum();
        assert_eq!(h.grass_values().len(), total + TIERS.len() * 5);
    }
}
