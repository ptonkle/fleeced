//! tool_shim: forwards strings between the page and `tool_core::api`.
//!
//! Builder requirement: the shim decides nothing. Each method is one call
//! into the core and a conversion of its arguments. The page's float clock
//! (milliseconds) becomes whole milliseconds. `crates/core/tests/boundary.rs`
//! fails if a branch, a loop or the `?` operator appears here; if a feature
//! seems to need one, the rule belongs in the core.

use wasm_bindgen::prelude::*;

/// The page's one session with the core.
#[wasm_bindgen]
pub struct Game {
    g: tool_core::api::Game,
}

#[wasm_bindgen]
impl Game {
    #[wasm_bindgen(constructor)]
    pub fn new(strings_json: &str, notes_json: &str, glossary_md: &str) -> Game {
        Game {
            g: tool_core::api::Game::new(strings_json, notes_json, glossary_md),
        }
    }

    pub fn ui(&self) -> String {
        self.g.ui()
    }

    pub fn act(&mut self, name: &str, value: &str) -> String {
        self.g.act(name, value)
    }

    pub fn net_event(&mut self, kind: &str, peer: &str, text: &str, host: bool, now: f64) {
        self.g.net_event(kind, peer, text, host, now as i64);
    }

    pub fn net_pump(&mut self, now: f64, keys: u32) -> String {
        self.g.net_pump(now as i64, keys)
    }

    pub fn look(&mut self, dx: f64, dy: f64) {
        self.g.look(dx as i32, dy as i32)
    }

    pub fn wheel(&mut self, delta: f64) {
        self.g.wheel(delta as i32)
    }

    pub fn scene_open(&mut self) -> String {
        self.g.scene_open()
    }

    pub fn scene_close(&mut self) {
        self.g.scene_close()
    }

    pub fn scene_resize(&mut self, css_w: f64, css_h: f64, dpr: f64) -> Vec<i32> {
        self.g
            .scene_resize(css_w as i32, css_h as i32, (dpr * 100.0) as i32)
    }

    pub fn scene_build(&mut self) {
        self.g.scene_build()
    }

    pub fn scene_values(&self, name: &str) -> Vec<f32> {
        floats(&self.g.scene_values(name))
    }

    pub fn scene_rows(&self, name: &str) -> Vec<u32> {
        self.g.scene_rows(name)
    }

    pub fn grass_vertices(&self) -> Vec<u8> {
        self.g.grass_vertices()
    }

    pub fn grass_indices(&self) -> Vec<u16> {
        self.g.grass_indices()
    }

    pub fn scene_sound(&self) -> String {
        self.g.scene_sound()
    }

    pub fn hud(&self) -> String {
        self.g.hud()
    }

    pub fn seen(&self) -> String {
        self.g.seen()
    }

    pub fn sound_setup(&self) -> String {
        self.g.sound_setup()
    }

    pub fn sound_noise(&self) -> Vec<u8> {
        self.g.sound_noise()
    }

    pub fn sound_master(&self, percent: i32) -> String {
        self.g.sound_master(percent)
    }
}

const ONE: f32 = tool_core::fx::ONE as f32;

fn floats(values: &[i32]) -> Vec<f32> {
    values.iter().map(|x| *x as f32 / ONE).collect()
}

/// The model gallery: the core's library and scenes, for `web/gallery.js`.
/// The core speaks Q16.16; WebGL wants `f32`, so values on the way out are
/// divided by the core's own `ONE`.
#[wasm_bindgen]
pub struct Gallery {
    g: tool_core::gallery::Gallery,
    frame: tool_core::render::Frame,
}

#[wasm_bindgen]
impl Gallery {
    #[wasm_bindgen(constructor)]
    pub fn new(query: &str) -> Gallery {
        Gallery {
            g: tool_core::gallery::Gallery::new(query),
            frame: Default::default(),
        }
    }

    pub fn numbers(&self) -> String {
        self.g.library().numbers()
    }

    pub fn keys(&self) -> String {
        tool_core::gallery::keys()
    }

    pub fn vertices(&self) -> Vec<f32> {
        floats(&self.g.library().vertices)
    }

    pub fn indices(&self) -> Vec<u32> {
        self.g.library().indices.clone()
    }

    pub fn poses(&self) -> Vec<f32> {
        floats(&self.g.library().poses.values)
    }

    /// CSS size and device pixel ratio in; drawing-buffer size out.
    pub fn resize(&mut self, css_w: f64, css_h: f64, dpr: f64) -> Vec<i32> {
        self.g
            .resize(css_w as i32, css_h as i32, (dpr * 100.0) as i32)
            .to_vec()
    }

    pub fn drag(&mut self, dx: f64, dy: f64) {
        self.g.drag(dx as i32, dy as i32)
    }

    pub fn wheel(&mut self, dy: f64) {
        self.g.wheel(dy as i32)
    }

    pub fn advance(&mut self, now_ms: f64) {
        self.g.advance((now_ms * 1000.0) as i64)
    }

    pub fn build(&mut self, panel: i32) {
        self.frame = self.g.frame_of(panel)
    }

    pub fn panels(&self) -> i32 {
        self.g.panels()
    }

    /// Where a panel goes on the canvas: x, y, width, height.
    pub fn panel_box(&self, panel: i32) -> Vec<i32> {
        self.g.panel_box(panel).to_vec()
    }

    /// A key from the page; whether the scene used it.
    pub fn key(&mut self, code: &str) -> bool {
        self.g.key(code)
    }

    pub fn hint(&self) -> String {
        self.g.hint().to_string()
    }

    pub fn seed(&self) -> i32 {
        self.g.seed()
    }

    pub fn tuning(&self) -> String {
        self.g.tuning()
    }

    pub fn labels(&self) -> String {
        self.g.labels()
    }

    pub fn view(&self) -> Vec<f32> {
        floats(&self.frame.view)
    }

    pub fn instances(&self) -> Vec<f32> {
        floats(&self.frame.instances)
    }

    pub fn draws(&self) -> Vec<u32> {
        self.frame.draws.clone()
    }

    pub fn outlines(&self) -> Vec<u32> {
        self.frame.outlines.clone()
    }
}

/// The hillside on its own, for `web/hillside.js`.
#[wasm_bindgen]
pub struct Hillside {
    h: tool_core::scape::view::Hillside,
    frame: tool_core::render::Frame,
}

#[wasm_bindgen]
impl Hillside {
    #[wasm_bindgen(constructor)]
    pub fn new(query: &str) -> Hillside {
        Hillside {
            h: tool_core::scape::view::Hillside::new(query),
            frame: Default::default(),
        }
    }

    /// The scene behind the menus.
    pub fn backdrop() -> Hillside {
        Hillside {
            h: tool_core::scape::view::Hillside::backdrop(),
            frame: Default::default(),
        }
    }

    pub fn stage(&mut self, json: &str) {
        self.h.set_stage(json)
    }

    pub fn due(&mut self, now_ms: f64) -> bool {
        self.h.due((now_ms * 1000.0) as i64)
    }

    pub fn still(&mut self, still: bool) {
        self.h.set_still(still)
    }

    pub fn bake(&mut self) {
        self.h.bake()
    }

    pub fn numbers(&self) -> String {
        self.h.numbers()
    }

    pub fn vertices(&self) -> Vec<f32> {
        floats(&self.h.library().vertices)
    }

    pub fn indices(&self) -> Vec<u32> {
        self.h.library().indices.clone()
    }

    pub fn poses(&self) -> Vec<f32> {
        floats(&self.h.library().poses.values)
    }

    #[wasm_bindgen(js_name = worldVertices)]
    pub fn world_vertices(&self) -> Vec<f32> {
        floats(&self.h.scape().bank.vertices)
    }

    #[wasm_bindgen(js_name = worldIndices)]
    pub fn world_indices(&self) -> Vec<u32> {
        self.h.scape().bank.indices.clone()
    }

    #[wasm_bindgen(js_name = worldInstances)]
    pub fn world_instances(&self) -> Vec<f32> {
        floats(&self.h.scape().instances)
    }

    #[wasm_bindgen(js_name = grassVertices)]
    pub fn grass_vertices(&self) -> Vec<u8> {
        self.h.grass().vertices.clone()
    }

    #[wasm_bindgen(js_name = grassIndices)]
    pub fn grass_indices(&self) -> Vec<u16> {
        self.h.grass_indices()
    }

    #[wasm_bindgen(js_name = groundPoints)]
    pub fn ground_points(&self) -> Vec<f32> {
        floats(&self.h.ground_points())
    }

    #[wasm_bindgen(js_name = groundIndices)]
    pub fn ground_indices(&self) -> Vec<u32> {
        self.h.scape().field.triangles()
    }

    pub fn resize(&mut self, css_w: f64, css_h: f64, dpr: f64) -> Vec<i32> {
        self.h
            .resize(css_w as i32, css_h as i32, (dpr * 100.0) as i32)
            .to_vec()
    }

    pub fn advance(&mut self, now_ms: f64) {
        self.h.advance((now_ms * 1000.0) as i64)
    }

    pub fn build(&mut self) {
        self.frame = self.h.frame()
    }

    pub fn view(&self) -> Vec<f32> {
        floats(&self.frame.view)
    }

    pub fn instances(&self) -> Vec<f32> {
        floats(&self.frame.instances)
    }

    pub fn draws(&self) -> Vec<u32> {
        self.frame.draws.clone()
    }

    pub fn outlines(&self) -> Vec<u32> {
        self.frame.outlines.clone()
    }

    #[wasm_bindgen(js_name = grassValues)]
    pub fn grass_values(&self) -> Vec<f32> {
        floats(&self.h.grass_values())
    }

    #[wasm_bindgen(js_name = grassRows)]
    pub fn grass_rows(&self) -> Vec<u32> {
        self.h.grass_rows()
    }
}

/// The sound test page's door into the core's sound desk.
#[wasm_bindgen]
pub struct SoundDesk {
    desk: tool_core::sound::Desk,
}

#[wasm_bindgen]
impl SoundDesk {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32) -> SoundDesk {
        SoundDesk {
            desk: tool_core::sound::Desk::new(seed as u64).expect("data/sound.json reads"),
        }
    }

    pub fn setup(&self) -> String {
        self.desk.setup()
    }

    pub fn listing(&self) -> String {
        self.desk.listing()
    }

    pub fn cue(&self, id: &str) -> String {
        self.desk.cue(id)
    }

    pub fn tune(&self, id: &str) -> String {
        self.desk.tune(id)
    }

    pub fn noise(&self) -> Vec<u8> {
        self.desk.noise()
    }

    pub fn master(&self, percent: i32) -> String {
        self.desk.master(percent as i64)
    }

    pub fn flock_frame(&mut self, count: i32) -> String {
        self.desk.flock_frame(count as i64)
    }
}
