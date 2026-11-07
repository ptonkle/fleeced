//! What the page's renderer draws, and every number it needs to read it.
//!
//! The core builds the meshes and the pose table once ([`Library`]) and the
//! page uploads them. A frame ([`Frame`]) is then small:
//! - the view values the shaders take as uniforms, laid out by [`UNIFORMS`];
//! - one instance per thing drawn: where it stands, which rows of the pose
//!   table it reads and its colors ([`Instance`]);
//! - the draws: one instanced draw per mesh, so 400 sheep are a few draw
//!   calls and the page does no per-sheep work;
//! - the outlines: instances drawn again through the colored outline pass.
//!
//! An animal's legs and head move in its vertex shader, from the pose table
//! in a float texture, so the core sends a row number per animal instead of
//! a matrix per bone. Every number the page needs is in [`numbers_json`], so
//! the page and the shaders hold none of their own. (The uniforms' layout
//! follows Honey Hole's `gfx/render.rs`; the look is this game's.)

pub mod geom;

use serde_json::json;

use crate::fx::{self, div, mul, ONE};
use crate::mesh::{self, rgb, Rgb};
use geom::{cm, deg, facing, lerp, v3, Affine, Mat4, V3};

pub const BYTES_PER_VALUE: usize = 4;
/// Values per instance: the placement's three columns and translation (12),
/// the pose (3), the coat and skin colors (6), the outline color and width
/// in pixels (4).
pub const INSTANCE_VALUES: usize = 25;
/// Values per draw: pass, bank, first index in bytes, index count, instance
/// source, byte offset of the first instance, instance count.
pub const DRAW_VALUES: usize = 7;

/// A draw's pass: a thing that does not bend.
pub const RIGID: u32 = 0;
/// A draw's pass: a thing moved by the pose table.
pub const SKINNED: u32 = 1;
/// A draw's pass: a soft dark shape blended onto the ground.
pub const DECAL: u32 = 2;
/// An outline's pass: drawn over everything, walls included.
pub const THROUGH: u32 = 3;
/// An outline's pass: hidden by what stands in front of it.
pub const IN_VIEW: u32 = 4;

/// Mesh banks: the library, then the world the map uploads when it builds.
pub const LIBRARY_BANK: u32 = 0;
pub const WORLD_BANK: u32 = 1;
/// Instance sources: sent every frame, or uploaded once with the world.
pub const FRAME_SOURCE: u32 = 0;
pub const WORLD_SOURCE: u32 = 1;

/// The uniforms in the view values, in order: name and size.
pub const UNIFORMS: &[(&str, usize)] = &[
    ("uVP", 16),
    ("uEye", 3),
    ("uFog", 2),
    ("uFogColor", 3),
    ("uSunDir", 3),
    ("uSun", 3),
    ("uSky", 3),
    ("uGround", 3),
    ("uSoft", 4),
    ("uZenith", 3),
    ("uSkyBand", 2),
    ("uSunDisc", 2),
    ("uSunColor", 3),
    ("uMoonDir", 3),
    ("uMoonDisc", 2),
    ("uMoonColor", 3),
    ("uStars", 3),
    ("uHash", 4),
    ("uFwd", 3),
    ("uRight", 3),
    ("uUp", 3),
    ("uPixel", 2),
    ("uShade", 1),
    ("uGradeR", 3),
    ("uGradeG", 3),
    ("uGradeB", 3),
];

/// Where a uniform sits in the view values, and its size.
pub fn uniform_at(name: &str) -> (usize, usize) {
    let mut at = 0;
    for (n, size) in UNIFORMS {
        if *n == name {
            return (at, *size);
        }
        at += size;
    }
    (at, 0)
}

/// A camera: where it is, where it looks, its vertical field of view
/// (turns) and its near and far planes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Camera {
    pub eye: V3,
    pub fwd: V3,
    pub fov: i32,
    pub near: i32,
    pub far: i32,
}

impl Camera {
    /// A camera `dist` from `target`, on the side `yaw` and `pitch` name.
    pub fn orbit(target: V3, yaw: i32, pitch: i32, dist: i32, fov: i32) -> Camera {
        let out = facing(yaw, pitch);
        Camera {
            eye: target.add(out.scale(dist)),
            fwd: out.neg(),
            fov,
            near: cm(10),
            far: fx::from_int(700),
        }
    }

    /// The play camera at this eye, looking this way. Its far plane reaches
    /// the hills (R1.902).
    pub fn along(eye: V3, fwd: V3) -> Camera {
        Camera {
            eye,
            fwd,
            fov: deg(52),
            near: cm(10),
            far: crate::scape::hills::REACH + fx::from_int(300),
        }
    }

    /// The play camera at rest: behind and above an animal at `at` heading
    /// along `yaw`, on the crosshair's line (`game/aim.rs`).
    pub fn chase(at: V3, yaw: i32) -> Camera {
        let way = facing(yaw, crate::game::aim::REST_PITCH as i32);
        let pivot = at.add(v3(0, crate::game::aim::PIVOT_UP, 0));
        Camera::along(pivot.sub(way.scale(crate::game::aim::BACK)), way)
    }

    /// The view-projection matrix for a screen `aspect` (width / height).
    pub fn matrix(&self, aspect: i32) -> Mat4 {
        Mat4::perspective(self.fov, aspect, self.near, self.far).mul(&Mat4::look(self.eye, self.fwd, V3::UP))
    }

    /// Where a world point lands on screen, in pixels from the top left of
    /// a screen of `size`; `None` behind the camera.
    pub fn screen(&self, size: [i32; 2], p: V3) -> Option<(i32, i32)> {
        let [x, y, _, w] = self.matrix(fx::ratio(size[0], size[1].max(1))).project(p);
        if w <= ONE / 100 {
            return None;
        }
        let half = (size[0] / 2, size[1] / 2);
        Some((
            half.0 + fx::round(mul(div(x, w), fx::from_int(half.0))),
            half.1 - fx::round(mul(div(y, w), fx::from_int(half.1))),
        ))
    }
}

/// The light and the air. Everything the sky pass and the soft light read;
/// the howl's quick night is [`Sky::between`] these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sky {
    pub sun_dir: V3,
    pub sun: Rgb,
    /// the fill from above and from below, blended by how a face points
    pub fill: Rgb,
    pub ground: Rgb,
    pub horizon: Rgb,
    pub zenith: Rgb,
    pub sun_color: Rgb,
    pub moon_dir: V3,
    pub moon_color: Rgb,
    pub fog: (i32, i32),
    /// how strongly shadows on the ground show
    pub shade: i32,
    /// how much of the star field shows, 0 by day
    pub stars: i32,
}

impl Sky {
    /// A soft high-latitude afternoon: a low, pale sun and a lot of sky.
    pub fn day() -> Sky {
        Sky {
            sun_dir: facing(deg(215), deg(42)),
            sun: rgb(118, 110, 92),
            fill: rgb(176, 188, 204),
            ground: rgb(118, 128, 96),
            horizon: rgb(214, 226, 232),
            zenith: rgb(118, 160, 206),
            sun_color: rgb(255, 248, 226),
            moon_dir: facing(deg(35), -deg(42)),
            moon_color: rgb(224, 230, 244),
            fog: (fx::from_int(120), fx::from_int(420)),
            shade: cm(52),
            stars: 0,
        }
    }

    /// The howl's night: the moon up, blue fill, the fog close in.
    pub fn night() -> Sky {
        Sky {
            sun_dir: facing(deg(215), -deg(30)),
            sun: rgb(52, 62, 96),
            fill: rgb(44, 54, 84),
            ground: rgb(20, 26, 40),
            horizon: rgb(24, 32, 56),
            zenith: rgb(8, 12, 28),
            sun_color: rgb(255, 196, 140),
            moon_dir: facing(deg(35), deg(46)),
            moon_color: rgb(236, 240, 252),
            fog: (fx::from_int(40), fx::from_int(170)),
            shade: cm(30),
            stars: ONE,
        }
    }

    /// `t` of the way from `self` to `other`. The sun and moon swing down
    /// and up through their own arcs, so dusk is a sunset, not a fade.
    pub fn between(&self, other: &Sky, t: i32) -> Sky {
        let dir = |a: V3, b: V3| a.lerp(b, t).norm();
        let col = |a: Rgb, b: Rgb| mesh::blend(a, b, t);
        Sky {
            sun_dir: dir(self.sun_dir, other.sun_dir),
            sun: col(self.sun, other.sun),
            fill: col(self.fill, other.fill),
            ground: col(self.ground, other.ground),
            horizon: col(self.horizon, other.horizon),
            zenith: col(self.zenith, other.zenith),
            sun_color: col(self.sun_color, other.sun_color),
            moon_dir: dir(self.moon_dir, other.moon_dir),
            moon_color: col(self.moon_color, other.moon_color),
            fog: (lerp(self.fog.0, other.fog.0, t), lerp(self.fog.1, other.fog.1, t)),
            shade: lerp(self.shade, other.shade, t),
            stars: lerp(self.stars, other.stars, t),
        }
    }
}

/// The dichromat's view for Dog mode: reds and greens fall together toward
/// a yellow-blue world, the shape of how a dog sees. Each row is what the
/// screen's red, green and blue take of the color drawn.
pub const DOG_MODE: [[i32; 3]; 3] = [[cm(62), cm(38), 0], [cm(62), cm(38), 0], [0, cm(30), cm(70)]];
pub const NO_GRADE: [[i32; 3]; 3] = [[ONE, 0, 0], [0, ONE, 0], [0, 0, ONE]];

/// The view values for a camera, a sky and a screen.
pub fn view_values(cam: &Camera, size: [i32; 2], sky: &Sky, dog_mode: bool) -> Vec<i32> {
    let aspect = fx::ratio(size[0], size[1].max(1));
    let mut v = cam.matrix(aspect).0.to_vec();
    let mut push = |a: &[i32]| v.extend_from_slice(a);
    let dir = |d: V3| [d.x, d.y, d.z];
    push(&dir(cam.eye));
    push(&[sky.fog.0, sky.fog.1]);
    push(&sky.horizon);
    push(&dir(sky.sun_dir));
    push(&sky.sun);
    push(&sky.fill);
    push(&sky.ground);
    // the sun wraps round the form (a scale and a bias on n·l), the fill
    // blends over the whole sphere
    push(&[cm(70), cm(30), ONE / 2, ONE / 2]);
    push(&sky.zenith);
    push(&[-cm(4), cm(46)]);
    // the discs: where the dot with the sun's direction starts and ends
    push(&[cm(99) + cm(1) * 72 / 100, cm(99) + cm(1) * 76 / 100]);
    push(&sky.sun_color);
    push(&dir(sky.moon_dir));
    push(&[cm(99) + cm(1) * 80 / 100, cm(99) + cm(1) * 84 / 100]);
    push(&sky.moon_color);
    // stars: how many, how small, how bright
    push(&[fx::from_int(140), cm(92), sky.stars]);
    // a star's place: the usual sine hash's numbers
    push(&[cm(1299), cm(7823), cm(3772), fx::from_int(4375)]);
    // the sky pass's rays
    let half = cam.fov / 2;
    let tan = div(fx::sin(half), fx::cos(half));
    let f = cam.fwd.norm();
    let right = f.cross(V3::UP).norm();
    let up = right.cross(f).norm();
    push(&dir(f));
    push(&dir(right.scale(mul(tan, aspect))));
    push(&dir(up.scale(tan)));
    // one pixel, in clip space (two units across the screen)
    push(&[fx::ratio(2, size[0].max(1)), fx::ratio(2, size[1].max(1))]);
    push(&[sky.shade]);
    let grade = if dog_mode { DOG_MODE } else { NO_GRADE };
    for row in grade {
        push(&row);
    }
    v
}

/// An outline: its color and its width in pixels, and whether it shows
/// through walls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outline {
    pub color: Rgb,
    pub width: i32,
    pub through: bool,
}

/// One thing drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instance {
    pub place: Affine,
    /// the pose table rows it reads, and how far from the first to the
    /// second; [0, 0, 0] is the table's still row, for what has no bones
    pub pose: [i32; 3],
    pub coat: Rgb,
    pub skin: Rgb,
}

impl Instance {
    /// A thing with no bones, in its own colors.
    pub fn still(place: Affine) -> Instance {
        Instance {
            place,
            pose: [0; 3],
            coat: [ONE; 3],
            skin: [ONE; 3],
        }
    }

    fn write(&self, outline: Option<Outline>, out: &mut Vec<i32>) {
        let a = &self.place;
        let (c, w) = outline.map_or(([0; 3], 0), |o| (o.color, fx::from_int(o.width)));
        out.extend_from_slice(&[
            a.c0.x, a.c0.y, a.c0.z, a.c1.x, a.c1.y, a.c1.z, a.c2.x, a.c2.y, a.c2.z, a.t.x, a.t.y, a.t.z,
        ]);
        out.extend_from_slice(&self.pose);
        out.extend_from_slice(&self.coat);
        out.extend_from_slice(&self.skin);
        out.extend_from_slice(&[c[0], c[1], c[2], w]);
    }
}

/// A mesh's place in the library's index buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshRange {
    pub first: u32,
    pub count: u32,
    pub pass: u32,
}

/// What one frame draws, sorted by mesh on its way out so each mesh is one
/// instanced draw. Ranges come from the library the frame draws from.
#[derive(Debug, Clone, Default)]
pub struct Batch {
    solid: Vec<Vec<i32>>,
    through: Vec<Vec<i32>>,
    in_view: Vec<Vec<i32>>,
}

impl Batch {
    pub fn new(meshes: usize) -> Batch {
        Batch {
            solid: vec![Vec::new(); meshes],
            through: vec![Vec::new(); meshes],
            in_view: vec![Vec::new(); meshes],
        }
    }

    /// One instance of mesh `mesh`, outlined if asked.
    pub fn push(&mut self, mesh: usize, i: &Instance, outline: Option<Outline>) {
        i.write(None, &mut self.solid[mesh]);
        if let Some(o) = outline {
            let list = if o.through {
                &mut self.through
            } else {
                &mut self.in_view
            };
            i.write(Some(o), &mut list[mesh]);
        }
    }

    /// Instances of mesh `mesh` so far.
    pub fn count_of(&self, mesh: usize) -> usize {
        self.solid[mesh].len() / INSTANCE_VALUES
    }

    /// The instance values, the draws and the outlines. Ground decals
    /// (`DECAL`) draw after everything solid.
    pub fn finish(self, ranges: &[MeshRange]) -> (Vec<i32>, Vec<u32>, Vec<u32>) {
        let mut values = Vec::new();
        let mut draws = Vec::new();
        let mut outlines = Vec::new();
        let emit = |list: &[i32], m: usize, pass: u32, into: &mut Vec<u32>, values: &mut Vec<i32>| {
            if list.is_empty() {
                return;
            }
            into.extend_from_slice(&[
                pass,
                LIBRARY_BANK,
                ranges[m].first * BYTES_PER_VALUE as u32,
                ranges[m].count,
                FRAME_SOURCE,
                (values.len() * BYTES_PER_VALUE) as u32,
                (list.len() / INSTANCE_VALUES) as u32,
            ]);
            values.extend_from_slice(list);
        };
        for keep in [false, true] {
            for (m, list) in self.solid.iter().enumerate() {
                if (ranges[m].pass == DECAL) == keep {
                    emit(list, m, ranges[m].pass, &mut draws, &mut values);
                }
            }
        }
        for (m, list) in self.through.iter().enumerate() {
            emit(list, m, THROUGH, &mut outlines, &mut values);
        }
        for (m, list) in self.in_view.iter().enumerate() {
            emit(list, m, IN_VIEW, &mut outlines, &mut values);
        }
        (values, draws, outlines)
    }
}

/// One frame, as the page reads it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Frame {
    pub view: Vec<i32>,
    pub instances: Vec<i32>,
    pub draws: Vec<u32>,
    pub outlines: Vec<u32>,
}

/// The buffer size for a CSS size and device pixel ratio (percent).
pub fn buffer_size(css_w: i32, css_h: i32, dpr_pct: i32) -> [i32; 2] {
    [(css_w * dpr_pct / 100).max(1), (css_h * dpr_pct / 100).max(1)]
}

/// Every number the page's renderer needs, as JSON. `poses` is the pose
/// texture's width and height in texels.
pub fn numbers_json(poses: (i32, i32)) -> String {
    let b = BYTES_PER_VALUE as u32;
    let calls = ["", "uniform1fv", "uniform2fv", "uniform3fv", "uniform4fv"];
    let uniforms: Vec<serde_json::Value> = UNIFORMS
        .iter()
        .filter(|(_, size)| *size < calls.len())
        .map(|(name, size)| json!([name, size, uniform_at(name).0, calls[*size]]))
        .collect();
    json!({
        "vertex": {
            "stride": mesh::VERTEX_VALUES as u32 * b,
            "attributes": [["aPos", 3, 0], ["aNormal", 3, 3 * b], ["aHull", 3, 6 * b], ["aColor", 3, 9 * b], ["aPaint", 3, 12 * b], ["aBone", 1, 15 * b]],
        },
        "instance": {
            "stride": INSTANCE_VALUES as u32 * b,
            "attributes": [["iC0", 3, 0], ["iC1", 3, 3 * b], ["iC2", 3, 6 * b], ["iT", 3, 9 * b], ["iPose", 3, 12 * b], ["iCoat", 3, 15 * b], ["iSkin", 3, 18 * b], ["iOutline", 4, 21 * b]],
        },
        "draw": DRAW_VALUES,
        "passes": { "rigid": RIGID, "skinned": SKINNED, "decal": DECAL, "through": THROUGH, "in_view": IN_VIEW },
        "banks": { "library": LIBRARY_BANK, "world": WORLD_BANK },
        "sources": { "frame": FRAME_SOURCE, "world": WORLD_SOURCE },
        "matrix": [0, 16],
        "uniforms": uniforms,
        "fog_color": [uniform_at("uFogColor").0, uniform_at("uFogColor").0 + 3],
        "poses": { "width": poses.0, "height": poses.1 },
        // the sky's one triangle, which covers the screen, and its size
        "sky": [-1, -1, 3, -1, -1, 3],
        "sky_size": 2,
    })
    .to_string()
}

/// A placement: standing at `at`, heading `yaw`, its up along `up` (the
/// ground's normal), `size` times its model.
pub fn stand(at: V3, yaw: i32, up: V3, size: i32) -> Affine {
    let u = up.norm();
    let ahead = v3(fx::sin(yaw), 0, fx::cos(yaw));
    let z = ahead.sub(u.scale(ahead.dot(u))).norm();
    let x = u.cross(z).norm();
    Affine {
        c0: x.scale(size),
        c1: u.scale(size),
        c2: z.scale(size),
        t: at,
    }
}

#[cfg(test)]
mod chase_tests {
    use super::*;

    #[test]
    fn the_chase_camera_stands_behind_and_above_and_looks_past_the_animal() {
        let at = v3(fx::from_int(10), 0, fx::from_int(20));
        let cam = Camera::chase(at, 0);
        assert!((cam.eye.y - fx::from_int(2)).abs() < cm(10), "{}", cam.eye.y);
        assert!((cam.eye.z - fx::from_int(15)).abs() < cm(30), "{}", cam.eye.z);
        assert_eq!(cam.eye.x, fx::from_int(10));
        assert!(cam.fwd.z > 0, "it looks the way the animal heads");
        assert!(cam.fwd.y < 0, "and down");
        let size = [1920, 1080];
        let (x, y) = cam.screen(size, at).expect("the animal is in front");
        assert_eq!(x, 960, "it stands in the middle of the screen");
        assert!(y > 540 && y < 1080, "and in the lower half, {y}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::from_int as int;

    #[test]
    fn the_uniforms_end_where_the_view_values_do() {
        let v = view_values(
            &Camera::orbit(V3::ZERO, 0, 0, int(5), deg(50)),
            [1920, 1080],
            &Sky::day(),
            false,
        );
        let total: usize = UNIFORMS.iter().map(|(_, s)| s).sum();
        assert_eq!(v.len(), total);
        let (at, size) = uniform_at("uGradeB");
        assert_eq!(at + size, total);
        assert_eq!(&v[at..at + size], &[0, 0, ONE], "no grade by default");
    }

    #[test]
    fn dog_mode_makes_red_and_green_the_same_on_screen() {
        let v = view_values(
            &Camera::orbit(V3::ZERO, 0, 0, int(5), deg(50)),
            [800, 600],
            &Sky::day(),
            true,
        );
        let (r, _) = uniform_at("uGradeR");
        let (g, _) = uniform_at("uGradeG");
        assert_eq!(v[r..r + 3], v[g..g + 3], "the screen's red and green rows match");
    }

    #[test]
    fn the_point_looked_at_is_the_middle_of_the_screen() {
        let c = Camera::orbit(v3(int(3), int(1), int(-2)), deg(30), deg(20), int(6), deg(50));
        let (x, y) = c.screen([1920, 1080], v3(int(3), int(1), int(-2))).unwrap();
        assert!((x - 960).abs() <= 1 && (y - 540).abs() <= 1, "{x} {y}");
        assert!(c.screen([1920, 1080], c.eye.sub(c.fwd.scale(int(3)))).is_none());
    }

    #[test]
    fn a_batch_makes_one_draw_per_mesh_with_its_decals_last() {
        let ranges = [
            MeshRange {
                first: 0,
                count: 30,
                pass: DECAL,
            },
            MeshRange {
                first: 30,
                count: 60,
                pass: SKINNED,
            },
        ];
        let mut b = Batch::new(2);
        let i = Instance::still(Affine::IDENTITY);
        b.push(0, &i, None);
        for _ in 0..400 {
            b.push(1, &i, None);
        }
        let (values, draws, outlines) = b.finish(&ranges);
        assert_eq!(values.len(), 401 * INSTANCE_VALUES);
        assert_eq!(draws.len(), 2 * DRAW_VALUES, "two draws for 401 things");
        assert_eq!(draws[0], SKINNED, "the solid mesh first");
        assert_eq!(draws[6], 400);
        assert_eq!(draws[7], DECAL, "the decal after it");
        assert_eq!(draws[9], 0, "first index in bytes");
        assert!(outlines.is_empty());
    }

    #[test]
    fn an_outlined_instance_goes_out_twice_and_keeps_its_width() {
        let ranges = [MeshRange {
            first: 6,
            count: 30,
            pass: SKINNED,
        }];
        let mut b = Batch::new(1);
        let red = Outline {
            color: [ONE, 0, 0],
            width: 3,
            through: true,
        };
        b.push(0, &Instance::still(Affine::IDENTITY), Some(red));
        let (values, draws, outlines) = b.finish(&ranges);
        assert_eq!(values.len(), 2 * INSTANCE_VALUES);
        assert_eq!(draws[6], 1);
        assert_eq!(outlines.len(), DRAW_VALUES);
        assert_eq!(outlines[0], THROUGH);
        assert_eq!(outlines[2], 6 * 4, "the mesh's first index in bytes");
        assert_eq!(outlines[5] as usize, INSTANCE_VALUES * 4, "after the solid copy");
        assert_eq!(values[2 * INSTANCE_VALUES - 1], int(3));
        assert_eq!(values[2 * INSTANCE_VALUES - 4], ONE, "red");
    }

    #[test]
    fn standing_on_a_slope_tilts_up_with_the_ground_and_keeps_the_size() {
        let up = v3(0, ONE, ONE / 4).norm();
        let a = stand(V3::ZERO, 0, up, ONE);
        assert_eq!(a.c1, up.scale(ONE));
        assert!((a.c2.len() - ONE).abs() < 8);
        assert_eq!(a.c2.dot(a.c1) / 4, 0, "still square");
        let big = stand(V3::ZERO, deg(90), V3::UP, 2 * ONE);
        assert!(
            (big.c2.x - 2 * ONE).abs() < 16,
            "heading 90 faces +x: {:?}",
            big.c2
        );
    }

    #[test]
    fn dusk_runs_from_the_day_to_the_night_and_the_sun_goes_down() {
        let (d, n) = (Sky::day(), Sky::night());
        assert_eq!(d.between(&n, 0).zenith, d.zenith);
        assert_eq!(d.between(&n, ONE).zenith, n.zenith);
        assert!(d.sun_dir.y > 0 && n.sun_dir.y < 0);
        assert!(
            d.between(&n, ONE * 3 / 4).sun_dir.y < 0,
            "under the horizon before the end"
        );
        assert!(n.moon_dir.y > 0 && d.moon_dir.y < 0);
    }

    #[test]
    fn numbers_lay_out_the_buffers_the_core_writes() {
        let n: serde_json::Value = serde_json::from_str(&numbers_json((69, 400))).unwrap();
        assert_eq!(n["instance"]["stride"], INSTANCE_VALUES * BYTES_PER_VALUE);
        assert_eq!(n["vertex"]["stride"], mesh::VERTEX_VALUES * BYTES_PER_VALUE);
        assert_eq!(n["draw"], DRAW_VALUES);
        // every instance attribute fits inside the stride
        let last = n["instance"]["attributes"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()
            .clone();
        assert_eq!(last[2].as_u64().unwrap() + last[1].as_u64().unwrap() * 4, 100);
        assert_eq!(n["poses"]["height"], 400);
    }
}
