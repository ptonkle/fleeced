//! The game's models, built by the core once at load: [`Library`] holds every
//! mesh in one vertex and index buffer, and the pose table the animals move
//! by. A scene pushes [`Animal`]s and props into a [`Batch`]; the far version
//! of a model is picked here by distance, so a flock stays cheap.
//!
//! Sizes follow `notes/contract.md`. The walk, run, crouch, jump and idle are
//! in `rig.rs`; a scene keeps a [`rig::Walker`] per animal and sends its pose.
//! Whatever decides whether a Wolf can hide is one table, `hiding.rs`.

pub mod canine;
pub mod hiding;
pub mod props;
pub mod rig;
pub mod sheep;
pub mod sheepdog;

use crate::fx::{self, mul, ONE, TURN};
use crate::mesh::{self, Rgb, Shape};
use crate::render::geom::{cm, deg, mm, v3, Affine, V3};
use crate::render::{self, Batch, Instance, MeshRange, Outline, DECAL, RIGID, SKINNED};

use hiding::{Hiding, HIDING};
use rig::{Clip, PoseTable, Rig, Walker, TEXELS_PER_BONE};

/// Past these distances a model draws its coarser meshes.
pub const MIDDLE_MESH: i32 = fx::from_int(12);
pub const FAR_MESH: i32 = fx::from_int(30);
/// Meshes per model: near, middle, far.
pub const LEVELS: usize = 3;

/// The models. Each has three levels of detail; only the crowd's differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Model {
    Ewe,
    Ram,
    Lamb,
    Sheepdog,
    Wolf,
    /// a Wolf dressed in the sheep's own wool (the Sheep Suit powerup)
    WolfInSuit,
    /// the collar and bell a sheep wears (the Bell powerup): drawn with the
    /// sheep's own pose
    BellOnEwe,
    BellOnLamb,
    Carcass,
    Tuft,
    Shadow,
    Plinth,
    Meadow,
    Stones,
}

impl Model {
    pub const ALL: [Model; 14] = [
        Model::Ewe,
        Model::Ram,
        Model::Lamb,
        Model::Sheepdog,
        Model::Wolf,
        Model::WolfInSuit,
        Model::BellOnEwe,
        Model::BellOnLamb,
        Model::Carcass,
        Model::Tuft,
        Model::Shadow,
        Model::Plinth,
        Model::Meadow,
        Model::Stones,
    ];

    pub fn rig(self) -> Rig {
        match self {
            Model::Lamb | Model::BellOnLamb => Rig::Lamb,
            Model::Sheepdog => Rig::Dog,
            Model::Wolf | Model::WolfInSuit => Rig::Wolf,
            _ => Rig::Sheep,
        }
    }

    fn pass(self) -> u32 {
        match self {
            Model::Shadow => DECAL,
            Model::Carcass | Model::Tuft | Model::Plinth | Model::Meadow | Model::Stones => RIGID,
            _ => SKINNED,
        }
    }

    /// The radii of its shadow on the ground: across and along.
    pub fn shadow_size(self) -> (i32, i32) {
        match self {
            Model::Ewe => (cm(52), cm(72)),
            Model::Ram => (cm(58), cm(78)),
            Model::Lamb => (cm(30), cm(42)),
            Model::Sheepdog => (cm(36), cm(72)),
            Model::Wolf | Model::WolfInSuit => (cm(44), cm(92)),
            _ => (cm(50), cm(50)),
        }
    }

    fn build(self, level: u32, h: &Hiding) -> Shape {
        match self {
            Model::Ewe => sheep::adult(false, level),
            Model::Ram => sheep::adult(true, level),
            Model::Lamb => sheep::lamb(level),
            Model::Sheepdog => sheepdog::sheepdog(),
            Model::Wolf => canine::wolf(h, false, level),
            Model::WolfInSuit => canine::wolf(h, true, level),
            Model::BellOnEwe => sheep::bell(Rig::Sheep, level),
            Model::BellOnLamb => sheep::bell(Rig::Lamb, level),
            Model::Carcass => props::carcass(),
            Model::Tuft => props::tuft(),
            Model::Shadow => props::shadow(),
            Model::Plinth => props::plinth(),
            Model::Meadow => props::meadow(),
            Model::Stones => props::stones(),
        }
    }

    /// Whether the coarser meshes are its own (sheep are the crowd).
    fn has_levels(self) -> bool {
        matches!(
            self,
            Model::Ewe
                | Model::Ram
                | Model::Lamb
                | Model::BellOnEwe
                | Model::BellOnLamb
                | Model::Wolf
                | Model::WolfInSuit
        )
    }
}

/// Which mesh of the library a model is, at a level of detail.
pub fn mesh_of(model: Model, level: usize) -> usize {
    model as usize * LEVELS + level
}

/// Every mesh and the pose table, as the page uploads them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Library {
    pub vertices: Vec<i32>,
    pub indices: Vec<u32>,
    pub ranges: Vec<MeshRange>,
    pub poses: PoseTable,
}

impl Library {
    pub fn build() -> Library {
        Library::build_with(&HIDING)
    }

    /// The library with a hiding table of its own, for the tuning scenes.
    pub fn build_with(h: &Hiding) -> Library {
        let mut lib = Library {
            vertices: Vec::new(),
            indices: Vec::new(),
            ranges: Vec::new(),
            poses: PoseTable::build_with(h),
        };
        for model in Model::ALL {
            lib.add(model.build(0, h), model.pass());
            for level in 1..LEVELS as u32 {
                if model.has_levels() {
                    lib.add(model.build(level, h), model.pass());
                } else {
                    let same = lib.ranges[lib.ranges.len() - 1];
                    lib.ranges.push(same);
                }
            }
        }
        lib
    }

    fn add(&mut self, shape: Shape, pass: u32) {
        let base = (self.vertices.len() / mesh::VERTEX_VALUES) as u32;
        let (vs, ix) = shape.finish();
        mesh::write_vertices(&vs, TEXELS_PER_BONE, &mut self.vertices);
        self.ranges.push(MeshRange {
            first: self.indices.len() as u32,
            count: ix.len() as u32,
            pass,
        });
        self.indices.extend(ix.iter().map(|i| i + base));
    }

    pub fn batch(&self) -> Batch {
        Batch::new(self.ranges.len())
    }

    /// The numbers the page reads for this library.
    pub fn numbers(&self) -> String {
        render::numbers_json((rig::POSE_WIDTH, self.poses.rows))
    }

    /// Triangles in a model's mesh.
    pub fn triangles(&self, model: Model, level: usize) -> u32 {
        self.ranges[mesh_of(model, level)].count / 3
    }
}

/// An animal to draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Animal {
    pub model: Model,
    pub at: V3,
    pub yaw: i32,
    /// the ground's normal, so it stands square on a slope
    pub up: V3,
    pub size: i32,
    pub pose: [i32; 3],
    pub coat: Rgb,
    pub skin: Rgb,
    pub outline: Option<Outline>,
    /// wearing the Bell
    pub bell: bool,
}

/// A soft shadow under a thing that stands on the ground.
pub fn push_shadow(b: &mut Batch, model: Model, at: V3, yaw: i32, up: V3, size: i32) {
    let (wide, long) = model.shadow_size();
    let lift = at.add(up.scale(mm(12)));
    let place =
        render::stand(lift, yaw, up, ONE).then(&Affine::scale(v3(mul(wide, size), ONE, mul(long, size))));
    b.push(mesh_of(Model::Shadow, 0), &Instance::still(place), None);
}

/// Pushes an animal, its shadow and whatever it wears. `eye` picks the mesh.
pub fn push_animal(b: &mut Batch, eye: V3, a: &Animal) {
    let place = render::stand(a.at, a.yaw, a.up, a.size);
    let dist = a.at.sub(eye).len();
    let level = if dist > FAR_MESH {
        2
    } else {
        (dist > MIDDLE_MESH) as usize
    };
    let i = Instance {
        place,
        pose: a.pose,
        coat: a.coat,
        skin: a.skin,
    };
    b.push(mesh_of(a.model, level), &i, a.outline);
    push_shadow(b, a.model, a.at, a.yaw, a.up, a.size);
    if a.bell {
        let worn = if a.model == Model::Lamb {
            Model::BellOnLamb
        } else {
            Model::BellOnEwe
        };
        b.push(mesh_of(worn, level), &i, None);
    }
}

/// One full turn of the turntable in this many ticks: 6 s at 60 ticks a
/// second, as Ape Race's showcase.
pub const TURNTABLE_TICKS: u32 = 360;

/// Who stands on the turntable: its model and colors, where, and whether it
/// is this player's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stand {
    pub model: Model,
    pub at: V3,
    pub coat: Rgb,
    pub skin: Rgb,
    /// this player's own: brought forward on the plinth
    pub yours: bool,
}

/// A Wolf's coat: the cream white of the flock it hides among.
pub fn wolf_coat() -> Rgb {
    sheep::COATS[WOLF_MATCH].wool
}
/// The place in `sheep::COATS` of the coat a Wolf wears.
pub const WOLF_MATCH: usize = HIDING.coat as usize;

/// A lobby character's colors.
pub fn character_coat(model: Model) -> (Rgb, Rgb) {
    match model {
        Model::Wolf | Model::WolfInSuit => (wolf_coat(), wolf_coat()),
        _ => ([ONE; 3], [ONE; 3]),
    }
}

/// The top of the plinth above the ground.
const PLINTH_TOP: i32 = cm(24);

/// Everyone on the lobby's turntable: each keeps turning in place, one full
/// turn in 6 s, staggered so they never all face the same way. This
/// player's own stands on the plinth.
pub fn turntable(b: &mut Batch, lib: &Library, stands: &[Stand], eye: V3, ticks: u32) {
    let turn = ((ticks % TURNTABLE_TICKS) as i64 * TURN as i64 / TURNTABLE_TICKS as i64) as i32;
    for (k, s) in stands.iter().enumerate() {
        let yaw = turn + TURN * k as i32 / 8 + deg(30);
        let rig = s.model.rig();
        let phase = fx::ratio(
            ((ticks + k as u32 * 37) % Clip::Idle.ticks() as u32) as i32,
            Clip::Idle.ticks(),
        );
        let walker = Walker::new(Clip::Idle, phase);
        let feet = if s.yours {
            s.at.add(v3(0, PLINTH_TOP, 0))
        } else {
            s.at
        };
        if s.yours {
            b.push(
                mesh_of(Model::Plinth, 0),
                &Instance::still(Affine::translate(s.at)),
                None,
            );
            push_shadow(b, Model::Plinth, s.at, 0, V3::UP, cm(290));
        }
        push_animal(
            b,
            eye,
            &Animal {
                model: s.model,
                at: feet,
                yaw,
                up: V3::UP,
                size: ONE,
                pose: walker.pose(rig, &lib.poses),
                coat: s.coat,
                skin: s.skin,
                outline: None,
                bell: false,
            },
        );
    }
}

/// Where up to eight seats stand in the lobby, and which seat is yours: a
/// shallow arc in two rows, Sheepdogs and Wolves mixed as the seats fall, with
/// your own stepped out in front at the arc's middle. The room to the left
/// is the lobby panel's.
pub fn lineup(seats: &[Model], yours: usize) -> Vec<Stand> {
    let others: Vec<usize> = (0..seats.len()).filter(|k| *k != yours).collect();
    let n = others.len() as i32;
    // a long back row; past five, two wings in front, either side of the
    // plinth, which keeps the middle of the front to itself
    let wings = if n > 5 { n / 3 } else { 0 };
    let back = n - wings;
    let wide = cm(195);
    others
        .iter()
        .enumerate()
        .map(|(i, seat)| {
            let i = i as i32;
            let at = if i < back {
                let x = wide * i - wide * (back - 1) / 2;
                v3(x, 0, -cm(260) - mul(x, x) / 14)
            } else {
                let x = wide * 5 / 4 * (2 * (i - back) - (wings - 1));
                v3(x, 0, -cm(20))
            };
            place(seats[*seat], at, false)
        })
        .chain(seats.get(yours).map(|m| place(*m, v3(0, 0, cm(230)), true)))
        .collect()
}

/// Where a winning team stands, up to seven, before the camera: one row of
/// up to four; past that, a row in front and a longer one behind, each seat
/// in a gap of the row before, as for a team photograph. No plinth, since
/// everyone here won.
pub fn victors(seats: &[Model]) -> Vec<Stand> {
    let n = seats.len() as i32;
    let front = if n > 4 { n / 2 } else { n };
    let wide = cm(170);
    let along = |k: i32, row: i32| wide * (2 * k - row + 1) / 2;
    seats
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let i = i as i32;
            let at = if i < n - front {
                v3(along(i, n - front), 0, -cm(190))
            } else {
                v3(along(i - (n - front), front), 0, 0)
            };
            place(*m, at, false)
        })
        .collect()
}

/// How a winner in this place on the team celebrates: every Sheepdog
/// bounces, and of the Wolves every other one stands over a carcass.
pub fn win_clip(model: Model, k: i32) -> Clip {
    if model.rig() == Rig::Wolf && k % 2 == 1 {
        Clip::Gloat
    } else {
        Clip::Cheer
    }
}

/// Where a winner in this place is in its clip. Each has its own length of
/// cycle, so a team drifts apart and no two keep time; `ticks` of `None` is
/// a still scene, and every clip holds its best moment.
pub fn win_phase(rig: Rig, clip: Clip, k: i32, ticks: Option<u32>) -> i32 {
    let Some(ticks) = ticks else {
        return clip.held(rig);
    };
    let cycle = (clip.ticks() + k * clip.ticks() / 9) as u32;
    fx::ratio(((ticks + k as u32 * 97) % cycle) as i32, cycle as i32)
}

/// How far a winner turns from the camera: a Sheepdog further, so its tail
/// shows.
fn win_turn(rig: Rig, k: i32) -> i32 {
    let left = k % 2 == 0;
    match rig {
        Rig::Dog if left => deg(36),
        Rig::Dog => -deg(32),
        _ if left => deg(18),
        _ => -deg(14),
    }
}

/// Tufts of wool on the wind past a winning team.
const TUFTS: i32 = 16;
/// Ticks one takes to cross, and how far it goes across.
const TUFT_TICKS: i32 = 300;
const TUFT_SPAN: i32 = cm(2400);
const TUFT_SIZE: i32 = ONE / 2;

/// Where a tuft is, from the team's middle: how far to the side, how far
/// toward the camera, and how high. They blow one way, and start again.
pub fn tuft_at(k: i32, ticks: u32) -> V3 {
    let cycle = TUFT_TICKS + k * 19;
    let t = fx::ratio(((ticks as i64 + k as i64 * 181) % cycle as i64) as i32, cycle);
    let toward = cm(((k * 173) % 500) - 120);
    let high = cm(30 + (k * 61) % 190) + mul(cm(22), fx::sin(t.wrapping_mul(3) + k * TURN / 5));
    v3(mul(TUFT_SPAN, t) - TUFT_SPAN / 2, high, toward)
}

/// A winning team facing `facing`, each turned a little out of line so no
/// two stand alike, with wool blowing past. `ticks` of `None` is a still
/// scene.
pub fn cheering(b: &mut Batch, lib: &Library, stands: &[Stand], eye: V3, ticks: Option<u32>, facing: i32) {
    let (ahead_x, ahead_z) = (fx::sin(facing), fx::cos(facing));
    for (k, s) in stands.iter().enumerate() {
        let k = k as i32;
        let rig = s.model.rig();
        let turn = win_turn(rig, k);
        let clip = win_clip(s.model, k);
        let phase = win_phase(rig, clip, k, ticks);
        push_animal(
            b,
            eye,
            &Animal {
                model: s.model,
                at: s.at,
                yaw: facing + turn,
                up: V3::UP,
                size: ONE,
                pose: Walker::new(clip, phase).pose(rig, &lib.poses),
                coat: s.coat,
                skin: s.skin,
                outline: None,
                bell: false,
            },
        );
        if clip == Clip::Gloat {
            let (x, z) = (fx::sin(facing + turn), fx::cos(facing + turn));
            let at = s.at.add(v3(mul(x, cm(100)), mm(5), mul(z, cm(100))));
            let place = render::stand(at, k * TURN / 7, V3::UP, ONE);
            b.push(mesh_of(Model::Carcass, 0), &Instance::still(place), None);
        }
    }
    let middle = stands.iter().fold(V3::default(), |sum, s| sum.add(s.at));
    let n = (stands.len() as i32).max(1);
    let middle = v3(middle.x / n, middle.y / n, middle.z / n);
    let clock = ticks.unwrap_or(0);
    for k in 0..TUFTS {
        let p = tuft_at(k, clock);
        let at = middle.add(v3(
            mul(ahead_z, p.x) + mul(ahead_x, p.z),
            p.y,
            mul(-ahead_x, p.x) + mul(ahead_z, p.z),
        ));
        let spin = (clock as i32 + k * 40).wrapping_mul(TURN / 200);
        let place = render::stand(at, spin, V3::UP, TUFT_SIZE);
        b.push(mesh_of(Model::Tuft, 0), &Instance::still(place), None);
    }
}

/// The lobby's camera on that line: it looks left of the line, so the line
/// stands right of the middle and the lobby panel has the left of the screen.
pub fn lineup_camera() -> render::Camera {
    let target = v3(-cm(200), cm(60), -cm(40));
    let mut cam = render::Camera::orbit(target, deg(2), deg(15), cm(1300), deg(32));
    cam.fwd = target.sub(cam.eye).norm();
    cam
}

fn place(model: Model, at: V3, yours: bool) -> Stand {
    let (coat, skin) = character_coat(model);
    Stand {
        model,
        at,
        coat,
        skin,
        yours,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::from_int as int;
    use crate::mesh::Vertex;

    fn shape_extent(model: Model) -> (i32, i32, i32) {
        let (vs, _) = model.build(0, &HIDING).finish();
        let top = vs.iter().map(|v| v.pos.y).max().unwrap();
        let low = vs.iter().map(|v| v.pos.y).min().unwrap();
        let z = |f: fn(&Vertex) -> i32| vs.iter().map(f).collect::<Vec<_>>();
        let zs = z(|v| v.pos.z);
        let length = zs.iter().max().unwrap() - zs.iter().min().unwrap();
        (top, low, length)
    }

    /// The point of a bone at rest, at this moment of a clip.
    fn moved(rig: Rig, clip: Clip, t: i32, bone: i32, at: V3) -> V3 {
        rig::pose(rig, clip, t)[bone as usize].apply(at)
    }

    #[test]
    fn a_team_of_three_stands_in_one_row_and_seven_in_two_staggered() {
        let at = |n: usize| -> Vec<(i32, i32)> {
            let stands = victors(&vec![Model::Wolf; n]);
            assert!(
                stands.iter().all(|s| !s.yours),
                "no plinth on the winners' screen"
            );
            stands.iter().map(|s| (s.at.x, s.at.z)).collect()
        };
        assert_eq!(at(3), [(-cm(170), 0), (0, 0), (cm(170), 0)]);
        let back = -cm(190);
        assert_eq!(
            at(7),
            [
                (-cm(255), back),
                (-cm(85), back),
                (cm(85), back),
                (cm(255), back),
                (-cm(170), 0),
                (0, 0),
                (cm(170), 0),
            ]
        );
    }

    #[test]
    fn a_sheepdog_leaves_the_ground_at_the_top_of_its_bounce_with_its_tail_going() {
        let sk = rig::skeleton(Rig::Dog);
        let foot = v3(sk.fore.x, 0, sk.fore.foot.0);
        let paw = |t| moved(Rig::Dog, Clip::Cheer, t, rig::leg_bone(0, 2), foot).y;
        // a quarter of the way through, the top of the hop
        assert!(paw(ONE / 4) > cm(15), "{}", paw(ONE / 4));
        let body = moved(Rig::Dog, Clip::Cheer, ONE / 4, rig::BODY, sk.body).y;
        assert!(body > sk.body.y + cm(10), "{body}");
        assert!(paw(0).abs() < cm(1), "{}", paw(0));
        let tip = sk.tail[2];
        let side = |t| moved(Rig::Dog, Clip::Cheer, t, rig::TAIL[2], tip).x;
        // three wags a cycle: a twelfth in, it swings one way, and three
        // twelfths in, the other
        assert!(side(ONE / 12) * side(ONE / 4) < 0);
        // the jaw is open as it barks at the top, and shut on the ground
        let jaw = |t| moved(Rig::Dog, Clip::Cheer, t, rig::JAW, sk.jaw.add(v3(0, 0, cm(8)))).y;
        let head = |t| moved(Rig::Dog, Clip::Cheer, t, rig::HEAD, sk.jaw.add(v3(0, 0, cm(8)))).y;
        assert!(head(ONE / 4) - jaw(ONE / 4) > cm(2));
        assert_eq!(head(0), jaw(0));
    }

    #[test]
    fn a_wolf_howls_again_and_again_and_gloats_with_its_head_low() {
        let sk = rig::skeleton(Rig::Wolf);
        let nose = |clip, t| moved(Rig::Wolf, clip, t, rig::HEAD, sk.jaw);
        // at the start of a howl nothing has moved: the jaw at rest
        let rest = sk.jaw.y;
        assert!(nose(Clip::Cheer, ONE / 2).y > rest + cm(15));
        assert!(Clip::Cheer.loops() && Clip::Gloat.loops());
        assert!(
            (nose(Clip::Cheer, 0).y - rest).abs() < mm(1),
            "back down between howls"
        );
        assert!(nose(Clip::Gloat, 0).y < rest - cm(15));
        assert_eq!(
            Clip::Cheer.held(Rig::Wolf),
            ONE / 2,
            "a still scene holds the howl"
        );
    }

    #[test]
    fn sheepdogs_all_cheer_and_every_other_wolf_stands_over_a_carcass() {
        let of = |m| -> Vec<Clip> { (0..4).map(|k| win_clip(m, k)).collect() };
        assert_eq!(of(Model::Sheepdog), [Clip::Cheer; 4]);
        assert_eq!(
            of(Model::Wolf),
            [Clip::Cheer, Clip::Gloat, Clip::Cheer, Clip::Gloat]
        );
    }

    #[test]
    fn no_two_winners_keep_time_and_a_still_scene_does_not_move() {
        for ticks in [0, 1000, 54321] {
            let mut phases: Vec<i32> = (0..7)
                .map(|k| win_phase(Rig::Dog, Clip::Cheer, k, Some(ticks)))
                .collect();
            phases.sort();
            phases.dedup();
            assert_eq!(phases.len(), 7, "at {ticks}");
        }
        assert_eq!(win_phase(Rig::Dog, Clip::Cheer, 3, None), 0);
        assert_eq!(win_phase(Rig::Wolf, Clip::Cheer, 3, None), ONE / 2);
    }

    #[test]
    fn wool_blows_one_way_across_and_comes_round_again() {
        let now = tuft_at(0, 0);
        assert_eq!(now.x, -TUFT_SPAN / 2);
        assert!(tuft_at(0, 150).x > now.x);
        assert_eq!(tuft_at(0, TUFT_TICKS as u32).x, now.x);
    }

    /// How high a model's body stands at a pose (ignoring what the head
    /// does): the top of the fleece or back, bones applied.
    fn body_top(model: Model, clip: Clip) -> i32 {
        let bones = rig::pose(model.rig(), clip, 0);
        let (vs, _) = model.build(0, &HIDING).finish();
        vs.iter()
            .filter(|v| v.bone == rig::BODY || v.bone == rig::CHEST)
            .map(|v| bones[v.bone as usize].apply(v.pos).y)
            .max()
            .unwrap()
    }

    #[test]
    fn the_models_stand_on_the_ground_at_the_contract_sizes() {
        // (model, length, shoulder), meters; the tail hangs outside the
        // length and the head may rise above the shoulder
        for (model, length, shoulder) in [
            (Model::Ewe, cm(105), cm(62)),
            (Model::Lamb, cm(60), cm(38)),
            (Model::Sheepdog, cm(95), cm(56)),
            (Model::Wolf, cm(125), cm(80)),
        ] {
            let (_, low, long) = shape_extent(model);
            assert!(low.abs() < cm(2), "{model:?} floats or sinks: {low}");
            assert!(
                (long - length).abs() < cm(14),
                "{model:?} length {}",
                long * 100 / ONE
            );
            let top = body_top(model, Clip::Idle);
            assert!(
                (top - shoulder).abs() < cm(4),
                "{model:?} shoulder {}",
                top * 100 / ONE
            );
        }
    }

    #[test]
    fn a_crouched_wolf_is_as_high_as_a_ewe() {
        let wolf = body_top(Model::Wolf, Clip::Crouch);
        let ewe = body_top(Model::Ewe, Clip::Idle);
        assert!(
            (wolf - ewe).abs() < cm(3),
            "wolf {} ewe {}",
            wolf * 100 / ONE,
            ewe * 100 / ONE
        );
        assert!(
            body_top(Model::Wolf, Clip::Idle) > ewe + cm(15),
            "and stands well above it"
        );
    }

    #[test]
    fn a_crouched_sheepdog_comes_down_to_the_contract() {
        let dog = body_top(Model::Sheepdog, Clip::Crouch);
        assert!((dog - cm(38)).abs() < cm(3), "{}", dog * 100 / ONE);
    }

    #[test]
    fn every_clip_keeps_a_standing_foot_on_the_ground() {
        // a planted foot's lowest point stays at the ground in every moving
        // clip, whatever the hips do
        for model in [Model::Ewe, Model::Lamb, Model::Sheepdog, Model::Wolf] {
            let rig = model.rig();
            let sk = rig::skeleton(rig);
            for clip in [Clip::Walk, Clip::Run, Clip::Stalk, Clip::Idle, Clip::Crouch] {
                let mut lowest = i32::MAX;
                for step in 0..16 {
                    let bones = rig::pose(rig, clip, step * ONE / 16);
                    for leg in 0..4 {
                        let shape = if leg < 2 { sk.fore } else { sk.hind };
                        let foot = v3(0, shape.foot.1, shape.foot.0);
                        let p = bones[rig::leg_bone(leg, 2) as usize].apply(foot);
                        lowest = lowest.min(p.y);
                    }
                }
                assert!(lowest.abs() < cm(3), "{model:?} {clip:?}: {}", lowest * 100 / ONE);
            }
        }
    }

    #[test]
    fn a_leg_keeps_its_length_in_a_stride() {
        // the solve bends a leg; it must not stretch one
        let sk = rig::skeleton(Rig::Dog);
        for step in 0..16 {
            let bones = rig::pose(Rig::Dog, Clip::Run, step * ONE / 16);
            let upper = bones[rig::leg_bone(2, 0) as usize];
            let lower = bones[rig::leg_bone(2, 1) as usize];
            let knee = v3(sk.hind.x, sk.hind.knee.1, sk.hind.knee.0);
            let gap = upper.apply(knee).sub(lower.apply(knee)).len();
            assert!(gap < cm(1), "the knee stays joined: {}", gap * 1000 / ONE);
        }
    }

    #[test]
    fn a_table_row_reads_its_clip_and_blends_across_the_end_of_a_loop() {
        let t = PoseTable::build();
        assert_eq!(t.values.len() as i32, t.rows * rig::POSE_WIDTH * 4);
        let start = t.row(Rig::Sheep, Clip::Walk, 0);
        assert_eq!(start & (ONE - 1), 0, "phase 0 is a whole row");
        assert_eq!(
            t.row(Rig::Sheep, Clip::Walk, ONE / 2),
            start + 8 * ONE,
            "half of 16 rows"
        );
        // a loop's phase wraps; a jump holds its last row
        assert_eq!(t.row(Rig::Sheep, Clip::Walk, ONE + ONE / 2), start + 8 * ONE);
        assert_eq!(
            t.row(Rig::Wolf, Clip::Jump, 3 * ONE),
            t.row(Rig::Wolf, Clip::Jump, ONE),
            "held"
        );
        // the row past a loop's end is its first moment again
        let width = (rig::POSE_WIDTH * 4) as usize;
        let row = |r: i32| &t.values[r as usize * width..(r as usize + 1) * width];
        let first = start >> fx::SHIFT;
        assert_eq!(row(first), row(first + Clip::Walk.steps()));
    }

    #[test]
    fn row_zero_is_still_and_walking_rows_differ() {
        let t = PoseTable::build();
        // every bone of the first row is the identity, three rows of four
        let still = [ONE, 0, 0, 0, 0, ONE, 0, 0, 0, 0, ONE, 0];
        for bone in t.values[..rig::BONE_COUNT * still.len()].chunks(still.len()) {
            assert_eq!(bone, still);
        }
        let walk = rig::pose(Rig::Sheep, Clip::Walk, ONE / 4);
        let other = rig::pose(Rig::Sheep, Clip::Walk, 3 * ONE / 4);
        assert_ne!(
            walk[rig::leg_bone(0, 0) as usize],
            other[rig::leg_bone(0, 0) as usize]
        );
    }

    #[test]
    fn a_stride_covers_the_ground_the_body_does() {
        let speed = int(2);
        let rig = Rig::Sheep;
        let mut w = Walker::new(Clip::Walk, 0);
        for _ in 0..60 {
            w.step(rig, Clip::Walk, speed);
        }
        // a second at 2 m/s is 2 / 0.95 of a cycle
        let want = fx::div(speed, rig::stride(rig, Clip::Walk));
        assert!((w.phase - want).abs() < ONE / 50, "{} {}", w.phase, want);
    }

    #[test]
    fn a_leg_reaches_as_far_as_its_stride_or_the_feet_would_skate() {
        // a foot on the ground goes back by stride × duty; a stride the leg
        // cannot reach leaves the foot sliding on the grass
        let (sheep, canine) = ((cm(40), cm(20)), (cm(33), cm(18)));
        for (model, (walk, run)) in [
            (Model::Ewe, sheep),
            (Model::Lamb, sheep),
            (Model::Sheepdog, canine),
            (Model::Wolf, canine),
        ] {
            let r = model.rig();
            let sk = rig::skeleton(r);
            let gaits = [
                (Clip::Walk, walk),
                (Clip::Run, run),
                (Clip::Stalk, cm(50)),
                (Clip::Drift, cm(70)),
            ];
            for (clip, duty) in gaits {
                let want = mul(rig::stride(r, clip), duty);
                for leg in 0..4 {
                    let shape = if leg < 2 { sk.fore } else { sk.hind };
                    let toe = v3(0, shape.foot.1, shape.foot.0);
                    let z = |k: i32| {
                        rig::pose(r, clip, k * ONE / 64)[rig::leg_bone(leg, 2) as usize]
                            .apply(toe)
                            .z
                    };
                    let hi = (0..64).map(z).max().unwrap();
                    let lo = (0..64).map(z).min().unwrap();
                    let got = hi - lo;
                    assert!(
                        (got - want).abs() < cm(5),
                        "{model:?} {clip:?} leg {leg}: reaches {}, wants {}",
                        got * 100 / ONE,
                        want * 100 / ONE
                    );
                }
            }
        }
    }

    #[test]
    fn every_row_of_the_hiding_table_changes_the_wolf() {
        let wolf = |h: &Hiding| {
            let (plain, _) = Model::Wolf.build(0, h).finish();
            let (disguised, _) = Model::WolfInSuit.build(0, h).finish();
            let poses = [Clip::Crouch, Clip::Idle].map(|c| rig::pose_with(h, Rig::Wolf, c, 0));
            (plain, disguised, poses)
        };
        let base = wolf(&HIDING);
        for (key, value) in HIDING.rows() {
            // the scene dresses the Wolf in this one
            if key == "coat" {
                continue;
            }
            let mut h = HIDING;
            h.set(key, value + 7);
            assert_ne!(wolf(&h), base, "{key} changes nothing");
        }
    }

    #[test]
    fn a_wolf_is_a_lean_wolf_with_short_fur_and_its_disguise_is_the_cloud() {
        // the trunk's corners between two places along it; the legs have
        // bones of their own
        let trunk = |model: Model, lo: i32, hi: i32| -> Vec<V3> {
            let (vertices, _) = model.build(0, &HIDING).finish();
            vertices
                .iter()
                .filter(|v| v.bone == rig::BODY || v.bone == rig::CHEST)
                .map(|v| v.pos)
                .filter(|p| p.z > cm(lo) && p.z < cm(hi))
                .collect()
        };
        let wide = |m: Model| trunk(m, -40, 30).iter().map(|p| p.x.abs()).max().unwrap();
        // a ewe's fleece is 54 cm across; a wolf's barrel is narrow
        assert!(wide(Model::Wolf) < cm(24), "{}", wide(Model::Wolf) * 100 / ONE);
        assert!(
            wide(Model::WolfInSuit) > cm(26),
            "{}",
            wide(Model::WolfInSuit) * 100 / ONE
        );
        let low = |lo, hi| trunk(Model::Wolf, lo, hi).iter().map(|p| p.y).min().unwrap();
        assert!(low(-30, -14) > low(8, 22) + cm(5), "the belly tucks up");
        let top = body_top(Model::Wolf, Clip::Idle);
        assert!((cm(78)..=cm(83)).contains(&top), "the withers stay put");
    }

    /// A mesh's darkest crease on the trunk, percent of the coat.
    fn trunk_shade(model: Model) -> i32 {
        let (vertices, _) = model.build(0, &HIDING).finish();
        let lowest = vertices
            .iter()
            .filter(|v| v.bone == rig::BODY)
            .map(|v| v.color[0])
            .min()
            .unwrap();
        lowest * 100 / ONE
    }

    #[test]
    fn the_disguise_wears_the_sheeps_soft_wool_and_keeps_its_snout() {
        // the soft wool never creases below 84 percent of the coat; hard clumps go to 70
        assert!(trunk_shade(Model::Ewe) >= 84);
        assert!(trunk_shade(Model::WolfInSuit) >= 84);
        let (ewe_faces, suit_faces) = (
            Model::Ewe.build(2, &HIDING).triangles(),
            Model::WolfInSuit.build(2, &HIDING).triangles(),
        );
        assert!(suit_faces < ewe_faces * 2, "as fine as the sheep: {suit_faces}");
        // the head starts 43 cm ahead of the middle: 17 cm of it is snout
        let reach = |model: Model| {
            let (vertices, _) = model.build(0, &HIDING).finish();
            vertices.iter().map(|v| v.pos.z).max().unwrap()
        };
        assert!(reach(Model::WolfInSuit) > cm(60), "{}", reach(Model::WolfInSuit));
    }

    #[test]
    fn the_bell_collar_sits_on_the_wool_and_not_in_it() {
        // (rig, the ruff's widest radius): the band's radius is 17 cm on a
        // ewe and 10.2 cm on a lamb, a little squashed
        for (kind, wool) in [(Rig::Sheep, cm(16)), (Rig::Lamb, cm(9))] {
            let sk = rig::skeleton(kind);
            let middle = sk.neck.lerp(sk.head, cm(38));
            let (vertices, _) = sheep::bell(kind, 0).finish();
            let band: Vec<_> = vertices.iter().filter(|v| v.bone == rig::NECK).collect();
            let sum: i64 = band.iter().map(|v| v.pos.sub(middle).len() as i64).sum();
            let mean = (sum / band.len() as i64) as i32;
            assert!(mean > wool, "{kind:?} {}", mean * 100 / ONE);
        }
    }

    #[test]
    fn a_sheep_keeps_its_eyes_and_nose_far_off_and_its_lids_near() {
        let colors = |level: u32| -> Vec<Rgb> {
            let (vertices, _) = sheep::adult(false, level).finish();
            vertices
                .iter()
                .filter(|v| v.bone == rig::HEAD)
                .map(|v| v.color)
                .collect()
        };
        for level in 0..LEVELS as u32 {
            let head = colors(level);
            assert!(head.contains(&sheep::EYE), "level {level} eye");
            assert!(head.contains(&sheep::NOSE), "level {level} nose");
        }
        assert!(colors(0).contains(&sheep::GLINT));
        assert!(!colors(2).contains(&sheep::GLINT), "no glint at thirty meters");
    }

    #[test]
    fn a_plain_wolf_has_a_masked_face_and_amber_eyes_and_its_disguise_does_not() {
        let colors = |model: Model, level: u32| -> Vec<Rgb> {
            let (vertices, _) = model.build(level, &HIDING).finish();
            vertices.iter().map(|v| v.color).collect()
        };
        for level in 0..LEVELS as u32 {
            let wolf = colors(Model::Wolf, level);
            assert!(wolf.contains(&canine::WOLF_EYE), "level {level}");
            assert!(wolf.contains(&canine::MASK), "level {level}");
        }
        assert!(colors(Model::Wolf, 0).contains(&canine::TOOTH));
        assert!(
            !colors(Model::Wolf, 2).contains(&canine::TOOTH),
            "no teeth past 30 m"
        );
        assert!(!colors(Model::WolfInSuit, 0).contains(&canine::MASK));
        assert!(!colors(Model::WolfInSuit, 0).contains(&canine::TOOTH));
    }

    #[test]
    fn a_plain_wolf_carries_its_head_level_with_its_shoulder() {
        // the mask's highest corner is the brow, the top of the skull
        let (vertices, _) = Model::Wolf.build(0, &HIDING).finish();
        let bones = rig::pose(Rig::Wolf, Clip::Idle, 0);
        let brow = vertices
            .iter()
            .filter(|v| v.bone == rig::HEAD && v.color == canine::MASK)
            .map(|v| bones[v.bone as usize].apply(v.pos).y)
            .max()
            .unwrap();
        let shoulder = body_top(Model::Wolf, Clip::Idle);
        assert!(brow < shoulder + cm(4), "brow {}", brow * 100 / ONE);
    }

    #[test]
    fn a_far_wolf_is_as_coarse_as_a_far_sheep() {
        let lib = Library::build();
        let (near, far) = (lib.triangles(Model::Wolf, 0), lib.triangles(Model::Wolf, 2));
        assert!(far * 4 < near * 3, "near {near}, far {far}");
    }

    #[test]
    fn a_change_of_clip_blends_out_of_the_old_one() {
        let t = PoseTable::build();
        let mut w = Walker::new(Clip::Idle, 0);
        w.step(Rig::Wolf, Clip::Run, int(7));
        let p = w.pose(Rig::Wolf, &t);
        assert!(p[2] > ONE / 2, "mostly the old clip at first: {}", p[2]);
        for _ in 0..12 {
            w.step(Rig::Wolf, Clip::Run, int(7));
        }
        assert_eq!(w.pose(Rig::Wolf, &t)[2], 0, "and all the new one soon");
    }

    #[test]
    fn a_far_flock_draws_the_far_mesh_at_a_fraction_of_the_triangles() {
        let lib = Library::build();
        let (near, far) = (lib.triangles(Model::Ewe, 0), lib.triangles(Model::Ewe, 2));
        assert!(near < 5000, "near ewe {near}");
        assert!(far < 1500 && far * 3 < near, "far ewe {far} of {near}");
    }

    #[test]
    fn the_far_mesh_is_chosen_by_distance() {
        let lib = Library::build();
        let ewe = Animal {
            model: Model::Ewe,
            at: v3(0, 0, int(40)),
            yaw: 0,
            up: V3::UP,
            size: ONE,
            pose: [0; 3],
            coat: [ONE; 3],
            skin: [ONE; 3],
            outline: None,
            bell: false,
        };
        let mut b = lib.batch();
        push_animal(&mut b, V3::ZERO, &ewe);
        assert_eq!(b.count_of(mesh_of(Model::Ewe, 2)), 1);
        assert_eq!(b.count_of(mesh_of(Model::Ewe, 0)), 0);
        push_animal(&mut b, v3(0, 0, int(20)), &ewe);
        assert_eq!(b.count_of(mesh_of(Model::Ewe, 1)), 1, "20 m off");
        push_animal(&mut b, v3(0, 0, int(35)), &ewe);
        assert_eq!(b.count_of(mesh_of(Model::Ewe, 0)), 1, "5 m off");
        assert_eq!(b.count_of(mesh_of(Model::Shadow, 0)), 3, "a shadow under each");
    }

    #[test]
    fn a_belled_sheep_wears_the_bell_in_its_own_pose() {
        let lib = Library::build();
        let ewe = Animal {
            model: Model::Lamb,
            at: V3::ZERO,
            yaw: 0,
            up: V3::UP,
            size: ONE,
            pose: [int(9), int(9), 0],
            coat: [ONE; 3],
            skin: [ONE; 3],
            outline: None,
            bell: true,
        };
        let mut b = lib.batch();
        push_animal(&mut b, V3::ZERO, &ewe);
        assert_eq!(b.count_of(mesh_of(Model::BellOnLamb, 0)), 1);
        let (values, draws, _) = b.finish(&lib.ranges);
        // the lamb's and the bell's instances carry the same pose
        let at = render::INSTANCE_VALUES;
        assert_eq!(values[12..15], values[at + 12..at + 15]);
        assert_eq!(draws.len(), 3 * render::DRAW_VALUES);
    }

    #[test]
    fn the_turntable_turns_a_full_circle_in_six_seconds() {
        let lib = Library::build();
        let seats = [Model::Sheepdog, Model::Wolf];
        let stands = lineup(&seats, 0);
        let at = |ticks: u32| {
            let mut b = lib.batch();
            turntable(&mut b, &lib, &stands, V3::ZERO, ticks);
            b.finish(&lib.ranges).0
        };
        // yaw only: the placement's first columns come back after 360 ticks
        assert_eq!(at(0)[..12], at(360)[..12]);
        assert_ne!(at(0)[..12], at(90)[..12], "a quarter turn is a different view");
    }

    #[test]
    fn your_own_character_stands_in_front_on_the_plinth() {
        let lib = Library::build();
        let seats = [
            Model::Sheepdog,
            Model::Sheepdog,
            Model::Sheepdog,
            Model::Sheepdog,
            Model::Wolf,
            Model::Wolf,
            Model::Wolf,
            Model::Wolf,
        ];
        let stands = lineup(&seats, 5);
        assert_eq!(stands.len(), 8, "every seat stands in the line");
        let yours: Vec<_> = stands.iter().filter(|s| s.yours).collect();
        assert_eq!(yours.len(), 1);
        assert_eq!(yours[0].model, Model::Wolf, "seat 5 is a Wolf");
        assert!(
            stands
                .iter()
                .filter(|s| !s.yours)
                .all(|s| s.at.z < yours[0].at.z - ONE),
            "in front of all the others"
        );
        let mut b = lib.batch();
        turntable(&mut b, &lib, &stands, V3::ZERO, 0);
        assert_eq!(b.count_of(mesh_of(Model::Plinth, 0)), 1, "one plinth");
    }

    #[test]
    fn seats_stand_apart_at_any_count() {
        for n in 1..=8 {
            let seats = vec![Model::Wolf; n];
            let stands = lineup(&seats, 0);
            assert_eq!(stands.len(), n);
            for (i, a) in stands.iter().enumerate() {
                for b in &stands[i + 1..] {
                    assert!(a.at.dist(b.at) > ONE, "{n} seats: {a:?} {b:?}");
                }
            }
        }
    }

    #[test]
    fn a_flock_is_mostly_cream_white_with_a_few_odd_sheep_and_the_wolf_wears_the_cream() {
        let cream = (0..1000)
            .filter(|i| sheep::coat_index(*i) < sheep::CREAM_COATS)
            .count();
        assert!((800..=900).contains(&cream), "{cream} of 1000 cream");
        let black = (0..1000).filter(|i| sheep::coat_index(*i) == 7).count();
        assert!((10..=60).contains(&black), "{black} of 1000 black");
        const { assert!(WOLF_MATCH < sheep::CREAM_COATS, "the majority's coat") };
        assert!(
            (0..400).all(|i| (sheep::size_of(i) - ONE).abs() <= cm(12)),
            "within 12%"
        );
        assert_eq!(wolf_coat(), sheep::COATS[WOLF_MATCH].wool);
    }

    #[test]
    fn cream_sheep_differ_a_little_in_tone_and_odd_sheep_keep_their_coat() {
        let tones: Vec<_> = (0..1000)
            .filter(|i| sheep::coat_index(*i) == 0)
            .map(|i| sheep::coat_of(i).wool)
            .collect();
        let base = sheep::COATS[0].wool;
        let near = |c: &crate::mesh::Rgb| (0..3).all(|k| (c[k] - base[k]).abs() <= cm(7));
        assert!(tones.iter().all(near), "a little, not a new color");
        let mut seen = tones.clone();
        seen.sort();
        seen.dedup();
        assert!(seen.len() > tones.len() / 2, "{} tones", seen.len());
        for i in (0..1000).filter(|i| sheep::coat_index(*i) >= sheep::CREAM_COATS) {
            assert_eq!(sheep::coat_of(i), sheep::COATS[sheep::coat_index(i)]);
        }
    }
}
