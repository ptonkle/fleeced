//! The model gallery (`web/gallery.html`): every model and test scene, built
//! from the page's query string, so a screenshot is a link. An orbit view
//! when the mouse moves it; `still=1` freezes a pose.
//!
//! `?model=ewe&clip=walk&phase=25&coat=4&view=three&still=1`
//! `?scene=hide` find the Wolf: 100 mixed sheep on plain grass from the play
//!   camera, with a crouched Wolf that moves like them. `N` or Space is a new
//!   seed, `R` outlines the Wolf, `T` opens the tuning panel. `dist=20` stands
//!   the player about that many meters from the Wolf, `view=above` looks down
//!   on it, `seed=3` picks the flock, `suit=1` puts the Wolf in its disguise.
//! `?scene=compare` a ewe of the Wolf's coat and a crouched Wolf from the top, the side and
//!   the play camera
//! `w.height=55` on either tries a row of the hiding table (`models/hiding.rs`)
//! `?scene=play` the play camera behind a Sheepdog, among sheep and a Wolf
//! `?scene=perf&n=400` the frame cost of a big flock
//! `?scene=disguise` the plain Wolf, the Wolf in its disguise and a ewe
//! `?scene=lineup`, `coats`, `outline`, `seats`
//! `&sky=night` or `dusk`, `&dog=1`, `&yaw=`, `&pitch=`, `&dist=`, `&fov=`

use serde_json::json;

use crate::fx::{self, from_int, mul, ONE, TURN};
use crate::mesh::Rgb;
use crate::models::hiding::{Hiding, HIDING};
use crate::models::rig::{Clip, Walker};
use crate::models::sheep::{coat_of, noise, size_of, COATS};
use crate::models::{
    self, character_coat, lineup, mesh_of, push_animal, turntable, Animal, Library, Model, Stand,
};
use crate::render::geom::{atan2, cm, deg, facing, v3, Affine, V3};
use crate::render::{buffer_size, view_values, Camera, Frame, Instance, Outline, Sky};

const MODELS: [(&str, Model); 8] = [
    ("ewe", Model::Ewe),
    ("ram", Model::Ram),
    ("lamb", Model::Lamb),
    ("sheepdog", Model::Sheepdog),
    ("wolf", Model::Wolf),
    ("suit", Model::WolfInSuit),
    ("carcass", Model::Carcass),
    ("plinth", Model::Plinth),
];

const CLIPS: [(&str, Clip); 13] = [
    ("idle", Clip::Idle),
    ("graze", Clip::Graze),
    ("walk", Clip::Walk),
    ("run", Clip::Run),
    ("crouch", Clip::Crouch),
    ("stalk", Clip::Stalk),
    ("drift", Clip::Drift),
    ("jump", Clip::Jump),
    ("snap", Clip::Snap),
    ("dig", Clip::Dig),
    ("howl", Clip::Howl),
    ("cheer", Clip::Cheer),
    ("gloat", Clip::Gloat),
];

const SCENES: [&str; 9] = [
    "lineup", "coats", "hide", "compare", "disguise", "play", "outline", "seats", "perf",
];

/// Ticks of grazing and of strolling, shortest and longest: 3 to 15 s, and
/// 0.7 to 3 s.
const GRAZE_TICKS: (i32, i32) = (180, 900);
const STROLL_TICKS: (i32, i32) = (42, 180);
/// The contract's graze drift, meters a second.
const DRIFT_SPEED: i32 = cm(40);
/// The most a strolling animal turns in a tick: 60 degrees a second.
const TURN_RATE: i32 = deg(1);
/// Ticks a new flock runs before it is shown, so its clips are mixed.
const WARM_UP_TICKS: i32 = 240;
/// How near the player an animal is dealt, meters.
const PLAYER_ROOM: i32 = from_int(4);
/// The red line of the reveal key.
const REVEAL: Outline = Outline {
    color: [ONE, 0, 0],
    width: 2,
    through: true,
};

/// What the page lists.
pub fn keys() -> String {
    json!({
        "models": MODELS.iter().map(|m| m.0).collect::<Vec<_>>(),
        "clips": CLIPS.iter().map(|c| c.0).collect::<Vec<_>>(),
        "scenes": SCENES,
    })
    .to_string()
}

/// The query's pairs.
struct Params(Vec<(String, String)>);

impl Params {
    fn parse(query: &str) -> Params {
        let pairs = query
            .trim_start_matches('?')
            .split('&')
            .filter_map(|p| p.split_once('='))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Params(pairs)
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    fn int(&self, key: &str, default: i32) -> i32 {
        self.get(key).and_then(|v| v.parse().ok()).unwrap_or(default)
    }

    fn on(&self, key: &str) -> bool {
        self.int(key, 0) != 0
    }
}

/// One animal in a gallery scene.
#[derive(Debug, Clone, Copy)]
struct Beast {
    model: Model,
    walker: Walker,
    clip: Clip,
    speed: i32,
    at: V3,
    yaw: i32,
    size: i32,
    coat: Rgb,
    skin: Rgb,
    outline: Option<Outline>,
    bell: bool,
    plan: Option<Plan>,
}

/// What an animal of the hide scene means to do. The Wolf draws from the same
/// plans as the sheep: that is how it moves like the flock.
#[derive(Debug, Clone, Copy)]
struct Plan {
    id: i32,
    drawn: i32,
    left: i32,
    strolling: bool,
    want: i32,
}

impl Plan {
    /// A repeatable roll from 0 to ONE for this draw.
    fn roll(&self, k: i32) -> i32 {
        (noise(self.id * 8191 + self.drawn * 131 + k) + ONE) / 2
    }
}

/// A repeatable pick from 0 up to `n`. The noise itself is always even, so its
/// remainder by an even number would skip half the picks.
fn pick(i: i32, n: i32) -> i32 {
    ((noise(i) + ONE) / 2).rem_euclid(n)
}

/// One tick of a beast's plan: grazing, or a slow stroll, turning for home
/// when it has gone past the flock's edge.
fn roam(b: &mut Beast, spread: i32) {
    let Some(mut plan) = b.plan else {
        return;
    };
    plan.left -= 1;
    if plan.left <= 0 {
        plan.drawn += 1;
        plan.strolling = !plan.strolling;
        let (low, high) = if plan.strolling { STROLL_TICKS } else { GRAZE_TICKS };
        plan.left = low + fx::floor(mul(plan.roll(1), from_int(high - low)));
        plan.want = if fx::length(b.at.x, b.at.z) > spread {
            atan2(-b.at.x, -b.at.z)
        } else {
            b.yaw + mul(plan.roll(2) * 2 - ONE, deg(70))
        };
    }
    if plan.strolling {
        let gap = (plan.want - b.yaw + TURN / 2).rem_euclid(TURN) - TURN / 2;
        b.yaw += gap.clamp(-TURN_RATE, TURN_RATE);
        b.clip = Clip::Drift;
        b.speed = DRIFT_SPEED;
        b.at = b.at.add(facing(b.yaw, 0).scale(DRIFT_SPEED / 60));
    } else {
        b.speed = 0;
        b.clip = if is_wolf(b.model) {
            Clip::Crouch
        } else if plan.roll(3) < ONE / 5 {
            Clip::Idle
        } else {
            Clip::Graze
        };
    }
    b.plan = Some(plan);
}

/// The find-the-Wolf scene's settings.
#[derive(Debug, Clone, Copy)]
struct Hide {
    seed: i32,
    /// how many sheep
    count: i32,
    /// how far from its middle the flock reaches
    spread: i32,
    /// the Wolf wears its disguise
    suit: bool,
    /// how far the player stands from the Wolf; unset, off the flock's edge
    dist: Option<i32>,
    above: bool,
    reveal: bool,
}

/// The default speed of a clip, meters a second (`notes/contract.md`).
fn speed_of(model: Model, clip: Clip) -> i32 {
    let sheep = matches!(model, Model::Ewe | Model::Ram | Model::Lamb);
    match (clip, sheep) {
        (Clip::Walk, true) => cm(240),
        (Clip::Run, true) => from_int(5),
        (Clip::Walk, false) => cm(450),
        (Clip::Run, false) => from_int(8),
        (Clip::Stalk, _) => cm(220),
        (Clip::Drift, _) => DRIFT_SPEED,
        _ => 0,
    }
}

fn beast(model: Model, clip: Clip, phase: i32, at: V3, yaw: i32) -> Beast {
    let (coat, skin) = character_coat(model);
    Beast {
        model,
        walker: Walker::new(clip, phase),
        clip,
        speed: speed_of(model, clip),
        at,
        yaw,
        size: ONE,
        coat,
        skin,
        outline: None,
        bell: false,
        plan: None,
    }
}

/// Whether a beast is the Wolf of the hide scene, disguised or not.
fn is_wolf(model: Model) -> bool {
    matches!(model, Model::Wolf | Model::WolfInSuit)
}

fn ewe(coat: usize, clip: Clip, phase: i32, at: V3, yaw: i32) -> Beast {
    Beast {
        coat: COATS[coat].wool,
        skin: COATS[coat].skin,
        ..beast(Model::Ewe, clip, phase, at, yaw)
    }
}

/// A prop to draw, in its own colors.
type Prop = (Model, Affine, Rgb);

/// The named camera sides: heading and pitch.
fn side_of(view: &str) -> (i32, i32) {
    match view {
        "front" => (0, deg(4)),
        "side" => (deg(90), deg(4)),
        "back" => (deg(180), deg(8)),
        "top" => (deg(180), deg(86)),
        _ => (deg(40), deg(18)),
    }
}

pub struct Gallery {
    lib: Library,
    hiding: Hiding,
    /// what the page writes over the view: text and where, percent of the view
    labels: Vec<(&'static str, i32, i32)>,
    hide: Option<Hide>,
    /// a camera per part of the canvas; none means one camera on all of it
    views: Vec<Camera>,
    beasts: Vec<Beast>,
    props: Vec<Prop>,
    stands: Vec<Stand>,
    sky: Sky,
    dog_mode: bool,
    size: [i32; 2],
    target: V3,
    yaw: i32,
    pitch: i32,
    dist: i32,
    fov: i32,
    /// a scene's fixed camera, until the mouse moves it
    eye: Option<V3>,
    spin: bool,
    still: bool,
    ticks: u32,
    next_us: i64,
}

impl Gallery {
    pub fn new(query: &str) -> Gallery {
        let p = Params::parse(query);
        let (yaw, pitch) = side_of(p.get("view").unwrap_or("three"));
        let sky = match p.get("sky") {
            Some("night") => Sky::night(),
            Some("dusk") => Sky::day().between(&Sky::night(), ONE * 55 / 100),
            _ => Sky::day(),
        };
        let mut hiding = HIDING;
        for (key, value) in &p.0 {
            if let (Some(row), Ok(n)) = (key.strip_prefix("w."), value.parse()) {
                hiding.set(row, n);
            }
        }
        let mut g = Gallery {
            lib: Library::build_with(&hiding),
            hiding,
            labels: Vec::new(),
            hide: None,
            views: Vec::new(),
            beasts: Vec::new(),
            props: Vec::new(),
            stands: Vec::new(),
            sky,
            dog_mode: p.on("dog"),
            size: [1920, 1080],
            target: v3(0, cm(45), 0),
            yaw: deg(p.int("yaw", 0)) + yaw,
            pitch: if p.get("pitch").is_some() {
                deg(p.int("pitch", 0))
            } else {
                pitch
            },
            dist: cm(p.int("dist", 260)),
            fov: deg(p.int("fov", 35)),
            eye: None,
            spin: p.on("spin"),
            still: p.on("still"),
            ticks: 0,
            next_us: 0,
        };
        g.props.push((Model::Meadow, Affine::IDENTITY, [ONE; 3]));
        match p.get("scene") {
            Some("lineup") => g.scene_lineup(&p),
            Some("coats") => g.scene_coats(&p),
            Some("hide") => g.scene_hide(&p),
            Some("compare") => g.scene_compare(),
            Some("play") => g.scene_play(),
            Some("disguise") => g.scene_disguise(&p),
            Some("outline") => g.scene_outline(&p),
            Some("seats") => g.scene_seats(&p),
            Some("perf") => g.scene_perf(&p),
            _ => g.scene_model(&p),
        }
        g
    }

    pub fn library(&self) -> &Library {
        &self.lib
    }

    fn clip_of(p: &Params) -> Clip {
        let name = p.get("clip").unwrap_or("idle");
        CLIPS.iter().find(|c| c.0 == name).map_or(Clip::Idle, |c| c.1)
    }

    fn scene_model(&mut self, p: &Params) {
        let name = p.get("model").unwrap_or("ewe");
        let model = MODELS.iter().find(|m| m.0 == name).map_or(Model::Ewe, |m| m.1);
        let clip = Self::clip_of(p);
        let phase = fx::ratio(p.int("phase", 0), 100);
        match model {
            Model::Carcass | Model::Plinth => {
                self.props
                    .push((model, Affine::IDENTITY, COATS[p.int("coat", 4) as usize % 8].wool));
                self.target = v3(0, cm(15), 0);
                self.dist = cm(p.int("dist", 220));
            }
            _ => {
                let mut b = beast(model, clip, phase, V3::ZERO, 0);
                if matches!(model, Model::Ewe | Model::Ram | Model::Lamb) {
                    let c = COATS[p.int("coat", 0) as usize % 8];
                    b.coat = c.wool;
                    b.skin = c.skin;
                }
                if model == Model::Wolf || model == Model::WolfInSuit {
                    b.skin = COATS[p.int("coat", 0) as usize % 8].wool;
                }
                b.bell = p.on("bell");
                self.target = v3(0, mul(cm(45), scale_for(model)), 0);
                self.dist = cm(p.int("dist", if model == Model::Wolf { 300 } else { 260 }));
                self.beasts.push(b);
            }
        }
    }

    /// Every body at its true size, side on, to check them against one
    /// another: the contract's table as a picture.
    fn scene_lineup(&mut self, p: &Params) {
        let row: [(Model, Clip); 7] = [
            (Model::Lamb, Clip::Idle),
            (Model::Ewe, Clip::Idle),
            (Model::Ram, Clip::Idle),
            (Model::Sheepdog, Clip::Idle),
            (Model::Sheepdog, Clip::Crouch),
            (Model::Wolf, Clip::Crouch),
            (Model::Wolf, Clip::Idle),
        ];
        let spacing = cm(165);
        for (k, (model, clip)) in row.iter().enumerate() {
            let at = v3(spacing * k as i32 - spacing * 3, 0, 0);
            let mut b = beast(*model, *clip, fx::ratio(k as i32, 7), at, deg(90));
            if matches!(model, Model::Ewe | Model::Ram | Model::Lamb) {
                b.coat = COATS[4].wool;
                b.skin = COATS[4].skin;
            }
            b.walker.phase = 0;
            self.beasts.push(b);
        }
        self.target = v3(0, cm(45), 0);
        self.yaw = 0;
        self.pitch = deg(p.int("pitch", 3));
        self.dist = cm(p.int("dist", 1100));
    }

    /// The breed's color range, with a ram and a lamb.
    fn scene_coats(&mut self, p: &Params) {
        let spacing = cm(130);
        for k in 0..8 {
            let at = v3(spacing * k - spacing * 7 / 2, 0, 0);
            let clip = if k % 2 == 0 { Clip::Idle } else { Clip::Graze };
            self.beasts
                .push(ewe(k as usize, clip, fx::ratio(k, 8), at, deg(70)));
        }
        let mut ram = ewe(6, Clip::Idle, 0, v3(-cm(100), 0, -cm(170)), deg(50));
        ram.model = Model::Ram;
        self.beasts.push(ram);
        let mut ram = ewe(0, Clip::Idle, ONE / 3, v3(cm(120), 0, -cm(170)), deg(120));
        ram.model = Model::Ram;
        self.beasts.push(ram);
        for (k, x) in [(0usize, cm(40)), (7, cm(280)), (3, -cm(300))] {
            let mut lamb = ewe(k, Clip::Idle, ONE / 2, v3(x, 0, cm(150)), deg(100));
            lamb.model = Model::Lamb;
            self.beasts.push(lamb);
        }
        self.target = v3(0, cm(40), 0);
        self.yaw = deg(p.int("yaw", 12));
        self.pitch = deg(p.int("pitch", 14));
        self.dist = cm(p.int("dist", 900));
    }

    /// The core trick: find the Wolf. A flock on plain grass, seen from the
    /// play camera, with a crouched Wolf that moves as the sheep do.
    fn scene_hide(&mut self, p: &Params) {
        self.hide = Some(Hide {
            seed: p.int("seed", 1),
            count: p.int("n", 100),
            spread: cm(p.int("spread", 1600)),
            suit: p.on("suit"),
            dist: p.get("dist").map(|_| from_int(p.int("dist", 20))),
            above: p.get("view") == Some("above"),
            reveal: p.on("reveal"),
        });
        self.deal();
    }

    /// Deals the flock and the Wolf for the seed, lets them settle into
    /// their plans, and stands the player where the scene asks.
    fn deal(&mut self) {
        let Some(h) = self.hide else {
            return;
        };
        let seed = h.seed;
        let slots = h.count + 1;
        let wolf = pick(seed * 31, slots);
        self.beasts.clear();
        for i in 0..slots {
            let far = fx::sqrt(fx::ratio(i * 2 + 1, slots * 2));
            let a = i * 25_033 + seed * 977;
            let wobble = |k: i32| mul(noise(i * k + seed), cm(130));
            let at = v3(
                mul(mul(fx::cos(a), far), h.spread) + wobble(3),
                0,
                mul(mul(fx::sin(a), far), h.spread) + wobble(5),
            );
            let yaw = (noise(i * 7 + seed * 3) + ONE) / 2;
            let phase = (noise(i * 11) + ONE) / 2;
            let mut b = if i == wolf {
                let body = if h.suit { Model::WolfInSuit } else { Model::Wolf };
                let mut w = beast(body, Clip::Crouch, phase, at, yaw);
                w.coat = COATS[self.hiding.coat as usize % COATS.len()].wool;
                w
            } else {
                let coat = coat_of(i * 3 + seed);
                let model = match pick(i * 29 + seed, 40) {
                    0..=4 => Model::Lamb,
                    5 => Model::Ram,
                    _ => Model::Ewe,
                };
                let mut s = beast(model, Clip::Graze, phase, at, yaw);
                s.size = size_of(i + seed);
                s.coat = coat.wool;
                s.skin = coat.skin;
                s
            };
            let rolled = (noise(i * 37 + seed) + ONE) / 2;
            b.plan = Some(Plan {
                id: i + seed * 211,
                drawn: 0,
                left: 1 + fx::floor(mul(rolled, from_int(GRAZE_TICKS.1))),
                strolling: i % 3 == 0,
                want: yaw,
            });
            self.beasts.push(b);
        }
        for _ in 0..WARM_UP_TICKS {
            self.step();
        }
        let wolf_at = self.beasts[wolf as usize].at;
        self.stand_player(&h, wolf_at);
        self.mark_wolf();
    }

    /// Puts the camera where the scene asks, the Wolf a little off the
    /// middle of the view, and makes room for the player among the sheep.
    fn stand_player(&mut self, h: &Hide, wolf: V3) {
        let heading = (noise(h.seed * 5) + ONE) / 2;
        let along = facing(heading, 0);
        let player = match h.dist {
            // a fifth nearer or further, so its row on screen gives nothing away
            Some(d) => wolf.sub(along.scale(d + mul(noise(h.seed * 7), d / 5))),
            None => along.scale(-from_int(26)),
        };
        for b in &mut self.beasts {
            let off = b.at.sub(player);
            let near = off.len();
            if near < PLAYER_ROOM && !is_wolf(b.model) {
                b.at = player.add(off.norm().scale(PLAYER_ROOM));
            }
        }
        let yaw = heading + mul(noise(h.seed * 13), deg(18));
        let cam = Camera::chase(player, yaw);
        let (eye, aim) = if h.above {
            (
                player.add(v3(0, from_int(14), 0)),
                player.add(facing(yaw, 0).scale(from_int(22))),
            )
        } else {
            (cam.eye, cam.eye.add(cam.fwd.scale(from_int(19))))
        };
        let look = aim.sub(eye);
        self.eye = Some(eye);
        self.target = aim;
        self.fov = cam.fov;
        // a drag starts from this same view
        self.dist = look.len();
        self.yaw = atan2(-look.x, -look.z);
        self.pitch = atan2(-look.y, fx::length(look.x, look.z));
    }

    fn mark_wolf(&mut self) {
        let on = self.hide.is_some_and(|h| h.reveal);
        for b in &mut self.beasts {
            if is_wolf(b.model) {
                b.outline = on.then_some(REVEAL);
            }
        }
    }

    /// A key from the page; whether the scene used it.
    pub fn key(&mut self, code: &str) -> bool {
        let Some(h) = self.hide.as_mut() else {
            return false;
        };
        match code {
            "KeyN" | "Space" => {
                h.seed += 1;
                h.reveal = false;
                self.deal();
                true
            }
            "KeyR" => {
                h.reveal = !h.reveal;
                self.mark_wolf();
                true
            }
            _ => false,
        }
    }

    /// The line the page shows along the bottom.
    pub fn hint(&self) -> &'static str {
        if self.hide.is_some() {
            "N new flock, R reveal, T tune"
        } else if self.views.len() < 2 {
            ""
        } else {
            "Top, side, play camera at 10 m and 20 m. T tune"
        }
    }

    /// The hide scene's seed, for the page to keep in the address.
    pub fn seed(&self) -> i32 {
        self.hide.map_or(0, |h| h.seed)
    }

    /// The hiding table as the tuning panel lists it.
    pub fn tuning(&self) -> String {
        let shown = self.hide.is_some() || !self.views.is_empty();
        let rows: Vec<_> = self
            .hiding
            .rows()
            .iter()
            .filter(|_| shown)
            .map(|(key, value)| json!({ "key": key, "value": value }))
            .collect();
        json!(rows).to_string()
    }

    /// A ewe of the Wolf's coat, a crouched Wolf and a grazing ewe in a row, from the
    /// top, the side, and the play camera at 10 m and 20 m. The play panels
    /// show the middle of the full screen at its true size, so a panel's
    /// animals are the size they are in play.
    fn scene_compare(&mut self) {
        let gap = cm(210);
        let coat = self.hiding.coat as usize % COATS.len();
        let mut wolf = beast(Model::Wolf, Clip::Crouch, 0, V3::ZERO, deg(90));
        wolf.coat = COATS[coat].wool;
        self.beasts
            .push(ewe(coat, Clip::Idle, 0, v3(-gap, 0, 0), deg(90)));
        self.beasts.push(wolf);
        self.beasts
            .push(ewe(coat, Clip::Graze, ONE / 3, v3(gap, 0, 0), deg(90)));
        let middle = v3(0, cm(35), 0);
        let near = |pitch: i32| Camera::orbit(middle, 0, pitch, cm(760), deg(30));
        let play = |meters: i32| {
            let mut cam = Camera::chase(v3(0, 0, -from_int(meters)), 0);
            let half = cam.fov / 2;
            cam.fov = 2 * atan2(fx::sin(half), 2 * fx::cos(half));
            cam
        };
        self.views = vec![near(deg(86)), near(deg(4)), play(10), play(20)];
    }

    /// The plain Wolf and the Wolf in its disguise, standing, with a ewe of the
    /// same coat between them.
    fn scene_disguise(&mut self, p: &Params) {
        let clip = Self::clip_of(p);
        let gap = cm(170);
        let yaw = -deg(30);
        self.beasts.push(beast(Model::Wolf, clip, 0, v3(-gap, 0, 0), yaw));
        self.beasts
            .push(ewe(models::WOLF_MATCH, Clip::Idle, 0, V3::ZERO, yaw));
        self.beasts
            .push(beast(Model::WolfInSuit, clip, 0, v3(gap, 0, 0), yaw));
        self.target = v3(0, cm(50), 0);
        self.yaw = 0;
        self.pitch = deg(p.int("pitch", 12));
        self.dist = cm(p.int("dist", 620));
    }

    /// The labels as the page writes them, in percent of the view.
    pub fn labels(&self) -> String {
        let cam = self.camera();
        let rows: Vec<_> = self
            .labels
            .iter()
            .filter_map(|(text, x, _)| {
                let (px, py) = cam.screen(self.size, v3(*x, cm(115), -cm(170)))?;
                let (left, top) = (px * 100 / self.size[0].max(1), py * 100 / self.size[1].max(1));
                Some(json!({ "text": text, "left": left, "top": top }))
            })
            .collect();
        json!(rows).to_string()
    }

    /// The play camera behind a Sheepdog, with a ewe beside it, and another
    /// Sheepdog, a far ewe and a standing Wolf further off, to tell them
    /// apart at a glance.
    fn scene_play(&mut self) {
        let dog = beast(Model::Sheepdog, Clip::Idle, 0, V3::ZERO, 0);
        self.beasts.push(dog);
        self.beasts
            .push(ewe(0, Clip::Idle, ONE / 4, v3(cm(170), 0, cm(100)), deg(30)));
        self.beasts
            .push(ewe(4, Clip::Graze, ONE / 3, v3(-cm(240), 0, cm(420)), deg(300)));
        let far = beast(
            Model::Sheepdog,
            Clip::Run,
            ONE / 8,
            v3(-cm(60), 0, cm(760)),
            deg(70),
        );
        self.beasts.push(far);
        let wolf = beast(Model::Wolf, Clip::Idle, 0, v3(cm(260), 0, cm(760)), deg(290));
        self.beasts.push(wolf);
        self.views = vec![Camera::chase(V3::ZERO, 0)];
    }

    /// Outlines through a wall: a green Sheepdog and a Wolf behind stones,
    /// a red target in the open, and a thing outlined only in plain view.
    fn scene_outline(&mut self, p: &Params) {
        let wall = Affine::translate(v3(0, 0, 0));
        self.props.push((Model::Stones, wall, [ONE; 3]));
        let green = Outline {
            color: [cm(40), ONE, cm(50)],
            width: 3,
            through: true,
        };
        let blue = Outline {
            color: [cm(50), cm(80), ONE],
            width: 3,
            through: true,
        };
        let red = Outline {
            color: [ONE, cm(16), cm(12)],
            width: 3,
            through: true,
        };
        let hidden = Outline {
            color: [ONE, cm(90), cm(30)],
            width: 3,
            through: false,
        };
        let mut dog = beast(
            Model::Sheepdog,
            Clip::Walk,
            ONE / 5,
            v3(-cm(130), 0, -cm(140)),
            deg(90),
        );
        dog.outline = Some(green);
        self.beasts.push(dog);
        let mut wolf = beast(Model::Wolf, Clip::Idle, 0, v3(cm(150), 0, -cm(170)), -deg(70));
        wolf.outline = Some(blue);
        self.beasts.push(wolf);
        let mut target = ewe(4, Clip::Graze, ONE / 3, v3(cm(80), 0, cm(220)), deg(120));
        target.outline = Some(red);
        self.beasts.push(target);
        let mut behind = ewe(2, Clip::Idle, 0, v3(-cm(20), 0, -cm(120)), deg(40));
        behind.outline = Some(hidden);
        self.beasts.push(behind);
        self.beasts
            .push(ewe(0, Clip::Idle, 0, v3(-cm(170), 0, cm(190)), deg(70)));
        self.target = v3(0, cm(60), 0);
        self.yaw = deg(p.int("yaw", 6));
        self.pitch = deg(p.int("pitch", 12));
        self.dist = cm(p.int("dist", 800));
    }

    /// The lobby: every seat turning, your own on the plinth. `you=` names
    /// your seat, `n=` how many seats (four Sheepdogs then four Wolves).
    fn scene_seats(&mut self, p: &Params) {
        let n = p.int("n", 8).clamp(1, 8) as usize;
        let seats: Vec<Model> = (0..n)
            .map(|k| if k < 4 { Model::Sheepdog } else { Model::Wolf })
            .collect();
        self.stands = lineup(&seats, p.int("you", 5) as usize % n);
        let cam = models::lineup_camera();
        self.eye = Some(cam.eye);
        self.target = cam.eye.add(cam.fwd);
        self.fov = cam.fov;
    }

    /// A big flock, half grazing, half walking, for the frame cost.
    fn scene_perf(&mut self, p: &Params) {
        let n = p.int("n", 400);
        let side = fx::floor(fx::sqrt(from_int(n))) + 1;
        for i in 0..n {
            let (a, b) = (i % side, i / side);
            let wobble = |k: i32| mul(noise(i * k), cm(60));
            let at = v3(
                cm(210) * a - cm(105) * side + wobble(3),
                0,
                cm(210) * b - cm(105) * side + wobble(5),
            );
            let clip = if i % 2 == 0 { Clip::Graze } else { Clip::Walk };
            let coat = coat_of(i);
            let phase = (noise(i * 11) + ONE) / 2;
            let mut s = beast(
                if i % 19 == 0 { Model::Lamb } else { Model::Ewe },
                clip,
                phase,
                at,
                noise(i * 7) / 2,
            );
            s.coat = coat.wool;
            s.skin = coat.skin;
            s.size = size_of(i);
            s.speed = cm(140);
            self.beasts.push(s);
        }
        self.target = v3(0, 0, 0);
        self.pitch = deg(p.int("pitch", 26));
        self.dist = cm(p.int("dist", 3400));
        self.fov = deg(p.int("fov", 50));
    }

    fn step(&mut self) {
        self.ticks += 1;
        let spread = self.hide.map_or(0, |h| h.spread);
        for b in &mut self.beasts {
            roam(b, spread);
            b.walker.step(b.model.rig(), b.clip, b.speed);
        }
        if self.spin {
            self.yaw += TURN / models::TURNTABLE_TICKS as i32;
        }
    }

    /// Runs the scene up to `now_us`, a tick at a time, at most ten behind.
    pub fn advance(&mut self, now_us: i64) {
        if self.still {
            return;
        }
        if self.next_us == 0 || now_us - self.next_us > 170_000 {
            self.next_us = now_us;
        }
        while self.next_us <= now_us {
            self.step();
            self.next_us += 16_667;
        }
    }

    pub fn resize(&mut self, css_w: i32, css_h: i32, dpr_pct: i32) -> [i32; 2] {
        self.size = buffer_size(css_w, css_h, dpr_pct);
        self.size
    }

    pub fn drag(&mut self, dx: i32, dy: i32) {
        self.eye = None;
        self.yaw -= dx * 90;
        self.pitch = (self.pitch + dy * 90).clamp(-deg(5), deg(88));
    }

    pub fn wheel(&mut self, dy: i32) {
        self.eye = None;
        self.dist = (self.dist + mul(self.dist, dy * ONE / 1000)).clamp(cm(50), from_int(120));
    }

    fn camera(&self) -> Camera {
        match self.eye {
            Some(eye) => {
                let mut cam = Camera::orbit(self.target, 0, 0, ONE, self.fov);
                cam.eye = eye;
                cam.fwd = self.target.sub(eye).norm();
                cam
            }
            None => Camera::orbit(self.target, self.yaw, self.pitch, self.dist, self.fov),
        }
    }

    /// How many parts of the canvas the scene draws.
    pub fn panels(&self) -> i32 {
        self.views.len().max(1) as i32
    }

    /// Where a panel goes: x and y from the bottom left, width and height.
    pub fn panel_box(&self, panel: i32) -> [i32; 4] {
        let [w, h] = self.size;
        if self.views.len() < 2 {
            return [0, 0, w, h];
        }
        // two by two, the first at the top left
        let (left, low) = (w / 2, h / 2);
        let (x, width) = if panel % 2 == 0 {
            (0, left)
        } else {
            (left, w - left)
        };
        let (y, height) = if panel < 2 { (low, h - low) } else { (0, low) };
        [x, y, width, height]
    }

    pub fn frame(&self) -> Frame {
        self.frame_of(0)
    }

    /// One panel's frame.
    pub fn frame_of(&self, panel: i32) -> Frame {
        let cam = self
            .views
            .get(panel as usize)
            .copied()
            .unwrap_or_else(|| self.camera());
        let [_, _, width, height] = self.panel_box(panel);
        let mut b = self.lib.batch();
        for (model, place, coat) in &self.props {
            let i = Instance {
                coat: *coat,
                ..Instance::still(*place)
            };
            b.push(mesh_of(*model, 0), &i, None);
        }
        for s in &self.beasts {
            push_animal(
                &mut b,
                cam.eye,
                &Animal {
                    model: s.model,
                    at: s.at,
                    yaw: s.yaw,
                    up: V3::UP,
                    size: s.size,
                    pose: s.walker.pose(s.model.rig(), &self.lib.poses),
                    coat: s.coat,
                    skin: s.skin,
                    outline: s.outline,
                    bell: s.bell,
                },
            );
        }
        turntable(&mut b, &self.lib, &self.stands, cam.eye, self.ticks);
        let (instances, draws, outlines) = b.finish(&self.lib.ranges);
        Frame {
            view: view_values(&cam, [width, height], &self.sky, self.dog_mode),
            instances,
            draws,
            outlines,
        }
    }
}

/// A model's height against a ewe, to frame it.
fn scale_for(model: Model) -> i32 {
    match model {
        Model::Wolf | Model::WolfInSuit => cm(140),
        Model::Lamb => cm(70),
        _ => ONE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render;

    #[test]
    fn a_query_picks_the_model_clip_and_view() {
        let g = Gallery::new("?model=wolf&clip=crouch&view=side&still=1");
        assert_eq!(g.beasts.len(), 1);
        assert_eq!(g.beasts[0].model, Model::Wolf);
        assert_eq!(g.beasts[0].clip, Clip::Crouch);
        assert_eq!(g.yaw, deg(90));
        assert!(g.still);
    }

    #[test]
    fn a_still_gallery_does_not_move_with_the_clock() {
        let mut g = Gallery::new("?model=ewe&clip=walk&still=1");
        let f = g.frame();
        g.advance(1_000_000);
        assert_eq!(g.frame(), f);
    }

    fn wolf_of(g: &Gallery) -> Beast {
        *g.beasts.iter().find(|b| b.model == Model::Wolf).unwrap()
    }

    #[test]
    fn the_hide_scene_holds_a_hundred_mixed_sheep_and_one_crouched_wolf() {
        let g = Gallery::new("?scene=hide&seed=4");
        let wolves = g.beasts.iter().filter(|b| b.model == Model::Wolf).count();
        assert_eq!(wolves, 1);
        assert_eq!(g.beasts.len(), 101);
        let w = wolf_of(&g);
        assert!(matches!(w.clip, Clip::Crouch | Clip::Drift), "{:?}", w.clip);
        assert_eq!(
            w.coat,
            COATS[models::WOLF_MATCH].wool,
            "its coat is the flock's cream"
        );
        assert!(w.outline.is_none(), "unmarked unless asked");
        let cream = |b: &&Beast| b.coat[2] > COATS[3].wool[0];
        let creams = g.beasts.iter().filter(cream).count();
        assert!((70..=99).contains(&creams), "mostly cream: {creams}");
        for m in [Model::Lamb, Model::Ram] {
            assert!(g.beasts.iter().any(|b| b.model == m), "{m:?}");
        }
    }

    #[test]
    fn the_disguise_scene_stands_the_plain_wolf_by_the_disguised_one() {
        let g = Gallery::new("?scene=disguise");
        let models: Vec<_> = g.beasts.iter().map(|b| b.model).collect();
        assert_eq!(models, [Model::Wolf, Model::Ewe, Model::WolfInSuit]);
    }

    #[test]
    fn the_new_seed_keys_deal_a_new_flock_and_put_the_wolf_elsewhere() {
        let mut g = Gallery::new("?scene=hide&seed=1");
        let before = wolf_of(&g).at;
        assert!(g.key("KeyN"));
        assert_eq!(g.seed(), 2);
        assert_ne!(wolf_of(&g).at, before);
        assert_eq!(g.beasts.len(), 101);
        assert!(g.key("Space"));
        assert_eq!(g.seed(), 3);
        assert!(!g.key("KeyQ"), "other keys are left to the page");
    }

    #[test]
    fn the_reveal_key_outlines_the_wolf_and_nothing_else() {
        let mut g = Gallery::new("?scene=hide");
        let marked = |g: &Gallery| g.beasts.iter().filter(|b| b.outline.is_some()).count();
        assert_eq!(marked(&g), 0);
        assert!(g.key("KeyR"));
        assert_eq!(marked(&g), 1);
        assert!(wolf_of(&g).outline.is_some());
        assert!(g.key("KeyR"));
        assert_eq!(marked(&g), 0);
        g.key("KeyR");
        g.key("KeyN");
        assert_eq!(marked(&g), 0, "a new flock is a new game");
    }

    #[test]
    fn the_wolf_moves_like_the_flock() {
        let mut g = Gallery::new("?scene=hide&seed=3");
        let index = |g: &Gallery| g.beasts.iter().position(|b| b.model == Model::Wolf).unwrap();
        let w = index(&g);
        let mut walked = vec![0; g.beasts.len()];
        for _ in 0..3600 {
            let before: Vec<V3> = g.beasts.iter().map(|b| b.at).collect();
            g.step();
            for (k, b) in g.beasts.iter().enumerate() {
                let step = b.at.sub(before[k]).len();
                assert!(step <= DRIFT_SPEED / 60 + 4, "{:?} stepped {step}", b.model);
                walked[k] += step;
            }
            let clip = g.beasts[w].clip;
            assert!(matches!(clip, Clip::Crouch | Clip::Drift), "{clip:?}");
        }
        // a minute of the same plans covers about the same ground
        let flock: i32 = walked.iter().sum::<i32>() / walked.len() as i32;
        assert!(flock > from_int(2), "the flock moved {flock}");
        assert!(
            walked[w] > flock / 3 && walked[w] < flock * 3,
            "{} {flock}",
            walked[w]
        );
    }

    #[test]
    fn the_flock_stays_together() {
        let mut g = Gallery::new("?scene=hide&seed=2");
        let spread = g.hide.unwrap().spread;
        for _ in 0..18_000 {
            g.step();
        }
        for b in &g.beasts {
            let out = fx::length(b.at.x, b.at.z);
            assert!(out < spread + from_int(5), "{:?} strayed to {out}", b.model);
        }
    }

    #[test]
    fn a_distance_stands_the_player_that_far_from_the_wolf_with_it_in_view() {
        let mut g = Gallery::new("?scene=hide&dist=20&seed=5");
        for _ in 0..4 {
            let cam = g.camera();
            let wolf = wolf_of(&g).at;
            let off = wolf.sub(cam.eye);
            // the camera stands 5 m behind the player, who is within a fifth
            let far = fx::length(off.x, off.z);
            assert!((far - from_int(25)).abs() < from_int(5), "{far}");
            let (x, y) = cam.screen([1920, 1080], wolf).expect("in front");
            assert!((1..1920).contains(&x) && (1..1080).contains(&y), "{x} {y}");
            g.key("KeyN");
        }
    }

    #[test]
    fn a_drag_starts_from_the_scene_s_own_view() {
        let mut g = Gallery::new("?scene=hide&dist=20");
        let held = g.camera();
        g.drag(0, 0);
        let cam = g.camera();
        assert!(
            cam.eye.sub(held.eye).len() < cm(20),
            "{:?} {:?}",
            cam.eye,
            held.eye
        );
    }

    #[test]
    fn a_table_row_in_the_address_reaches_the_wolf() {
        let g = Gallery::new("?scene=hide&w.height=50&w.coat=2&w.wings=3&w.width=x");
        assert_eq!(g.hiding.height, 50);
        assert_eq!(g.hiding.width, HIDING.width, "not a number");
        assert_ne!(g.lib.poses, Library::build().poses);
        assert_eq!(wolf_of(&g).coat, COATS[2].wool);
        assert!(
            g.tuning().contains("\"key\":\"height\",\"value\":50"),
            "{}",
            g.tuning()
        );
        let head = format!("\"key\":\"head\",\"value\":{}", HIDING.head);
        assert!(g.tuning().contains(&head), "{}", g.tuning());
        assert_eq!(Gallery::new("?model=ewe").tuning(), "[]");
    }

    #[test]
    fn the_compare_panels_fill_the_canvas_with_the_play_camera_at_its_true_size() {
        let mut g = Gallery::new("?scene=compare&still=1");
        g.size = [1921, 1081];
        assert_eq!(g.panels(), 4);
        let boxes: Vec<[i32; 4]> = (0..4).map(|k| g.panel_box(k)).collect();
        assert_eq!(boxes[0], [0, 540, 960, 541], "the first at the top left");
        assert_eq!(boxes[3], [960, 0, 961, 540]);
        let area: i32 = boxes.iter().map(|b| b[2] * b[3]).sum();
        assert_eq!(area, 1921 * 1081);
        // a one-meter post by the Wolf is as many pixels in the half-height
        // panel as on the whole screen
        let tall = |cam: &Camera, size: [i32; 2]| {
            let top = cam.screen(size, v3(0, ONE, 0)).unwrap().1;
            cam.screen(size, V3::ZERO).unwrap().1 - top
        };
        let whole = tall(&Camera::chase(v3(0, 0, -from_int(10)), 0), [1920, 1080]);
        let panel = tall(&g.views[2], [960, 540]);
        assert!((whole - panel).abs() <= 2, "{whole} {panel}");
        let f = g.frame_of(1);
        assert_ne!(f.view, g.frame_of(2).view);
        assert_eq!(g.beasts.len(), 3);
    }

    #[test]
    fn four_hundred_sheep_are_a_handful_of_draws() {
        let g = Gallery::new("?scene=perf&n=400");
        let f = g.frame();
        let draws = f.draws.len() / render::DRAW_VALUES;
        assert!(draws <= 6, "{draws} draws");
        // the ground, the ewes and lambs at each level, the shadows
        assert_eq!(f.instances.len(), (1 + 400 + 400) * render::INSTANCE_VALUES);
    }

    #[test]
    fn the_seats_scene_draws_every_seat_with_one_plinth() {
        let g = Gallery::new("?scene=seats&you=2");
        assert_eq!(g.stands.len(), 8);
        assert_eq!(g.stands.iter().filter(|s| s.yours).count(), 1);
    }

    #[test]
    fn an_unknown_model_falls_back_to_a_ewe() {
        let g = Gallery::new("?model=dragon");
        assert_eq!(g.beasts[0].model, Model::Ewe);
    }
}
