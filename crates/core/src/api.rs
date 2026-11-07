//! The door into the core. The shim holds one `Game` and forwards its calls;
//! each returns a JSON string, `{"ok": ...}` on success or `{"error": "..."}`
//! with a message the page can show as it is.
//!
//! The core describes the screen as a view model (`view::Ui`): the screen's
//! name, its panels and their items, and the glossary's definitions. The page
//! builds the DOM from that description without knowing what a panel means.
//! Buttons, settings, flags and screens all come back through one function,
//! `act`, with a dotted name such as `menu.settings`, so a new option needs a
//! new `match` arm in `app.rs` and no new shim function. (After Peter's
//! Mandible; the session held here rather than a state round-trip through the
//! page is Honey Hole's.)

use serde::Serialize;

use crate::app::App;
use crate::scene::Scene;
use crate::sound::Desk;

fn ok<T: Serialize>(value: &T) -> String {
    serde_json::json!({ "ok": value }).to_string()
}

fn error(message: &str) -> String {
    serde_json::json!({ "error": message }).to_string()
}

/// The app for one page, or why it could not start.
#[derive(Debug)]
pub struct Game {
    app: Result<App, String>,
}

impl Game {
    /// Checks the strings file, the patch notes and the glossary and opens
    /// on the Title.
    pub fn new(strings_json: &str, notes_json: &str, glossary_md: &str) -> Game {
        Game {
            app: App::new(strings_json, notes_json, glossary_md),
        }
    }

    /// What the page draws now.
    pub fn ui(&self) -> String {
        match &self.app {
            Ok(app) => ok(&app.ui()),
            Err(e) => error(e),
        }
    }

    /// Applies one named action and returns what the page draws next. A
    /// refused action leaves the app as it was.
    pub fn act(&mut self, name: &str, value: &str) -> String {
        match &mut self.app {
            Ok(app) => match app.act(name, value) {
                Ok(()) => ok(&app.ui()),
                Err(e) => error(&e),
            },
            Err(e) => error(e),
        }
    }

    /// The mouse moved in a match, in pixels.
    pub fn look(&mut self, dx: i32, dy: i32) {
        if let Ok(app) = &mut self.app {
            app.look(dx, dy);
        }
    }

    /// The mouse wheel turned in a match, as the page's delta.
    pub fn wheel(&mut self, delta: i32) {
        if let Ok(app) = &mut self.app {
            app.wheel(delta);
        }
    }

    /// Builds the match's scene and returns the numbers the page's renderer
    /// needs, or "" with no match to show.
    pub fn scene_open(&mut self) -> String {
        match &mut self.app {
            Ok(app) => app.open_scene().map_or_else(String::new, Scene::numbers),
            Err(_) => String::new(),
        }
    }

    pub fn scene_close(&mut self) {
        if let Ok(app) = &mut self.app {
            app.close_scene();
        }
    }

    pub fn scene_resize(&mut self, css_w: i32, css_h: i32, dpr_pct: i32) -> Vec<i32> {
        match &mut self.app {
            Ok(app) => app.resize_scene(css_w, css_h, dpr_pct).to_vec(),
            Err(_) => Vec::new(),
        }
    }

    /// Builds this frame. The page then reads it by name.
    pub fn scene_build(&mut self) {
        if let Ok(app) = &mut self.app {
            app.build_scene();
        }
    }

    fn scene(&self) -> Option<&Scene> {
        self.app.as_ref().ok().and_then(App::scene)
    }

    /// One of the scene's lists of `Fx` values, by name.
    pub fn scene_values(&self, name: &str) -> Vec<i32> {
        let Some(s) = self.scene() else {
            return Vec::new();
        };
        match name {
            "vertices" => s.hill().library().vertices.clone(),
            "poses" => s.hill().library().poses.values.clone(),
            "world_vertices" => s.hill().scape().bank.vertices.clone(),
            "world_instances" => s.hill().scape().instances.clone(),
            "ground_points" => s.hill().ground_points(),
            "view" => s.frame().view.clone(),
            "instances" => s.frame().instances.clone(),
            "grass_values" => s.hill().grass_values(),
            _ => Vec::new(),
        }
    }

    /// One of the scene's lists of whole numbers, by name.
    pub fn scene_rows(&self, name: &str) -> Vec<u32> {
        let Some(s) = self.scene() else {
            return Vec::new();
        };
        match name {
            "indices" => s.hill().library().indices.clone(),
            "world_indices" => s.hill().scape().bank.indices.clone(),
            "ground_indices" => s.hill().scape().field.triangles(),
            "draws" => s.frame().draws.clone(),
            "outlines" => s.frame().outlines.clone(),
            "grass_rows" => s.hill().grass_rows(),
            _ => Vec::new(),
        }
    }

    pub fn grass_vertices(&self) -> Vec<u8> {
        self.scene()
            .map_or_else(Vec::new, |s| s.hill().grass().vertices.clone())
    }

    pub fn grass_indices(&self) -> Vec<u16> {
        self.scene().map_or_else(Vec::new, |s| s.hill().grass_indices())
    }

    /// What the page plays this frame, for `web/audio.js`'s `playFrame`.
    pub fn scene_sound(&self) -> String {
        self.scene().map_or_else(String::new, Scene::sound)
    }

    /// The match's HUD for this browser.
    pub fn hud(&self) -> String {
        match &self.app {
            Ok(app) => ok(&app.hud()),
            Err(e) => error(e),
        }
    }

    /// What this viewer sees, for the browser checks.
    pub fn seen(&self) -> String {
        match &self.app {
            Ok(app) => ok(&app.seen()),
            Err(e) => error(e),
        }
    }

    /// What `startAudio` takes, besides the noise.
    pub fn sound_setup(&self) -> String {
        Desk::new(0).map_or_else(|e| error(&e), |d| d.setup())
    }

    pub fn sound_noise(&self) -> Vec<u8> {
        Desk::new(0).map_or_else(|_| Vec::new(), |d| d.noise())
    }

    /// The main gain for a sound level, percent.
    pub fn sound_master(&self, percent: i32) -> String {
        Desk::new(0).map_or_else(|e| error(&e), |d| d.play_master(percent as i64))
    }

    /// One event from the transport (`web/online.js`).
    pub fn net_event(&mut self, kind: &str, peer: &str, text: &str, host: bool, now_ms: i64) {
        if let Ok(app) = &mut self.app {
            app.net_event(kind, peer, text, host, now_ms);
        }
    }

    /// One display frame: what to send, and what to ask of the transport.
    pub fn net_pump(&mut self, now_ms: i64, keys: u32) -> String {
        match &mut self.app {
            Ok(app) => ok(&app.pump(now_ms, keys)),
            Err(e) => error(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strings::tests::sample;

    fn notes() -> String {
        let version = crate::app::VERSION;
        format!(r#"[{{"version": "{version}", "date": "2026-11-06", "notes": ["A."]}}]"#)
    }
    const GLOSSARY: &str = "### Fold\n\nWhere the sheep go.\n";

    fn json(text: &str) -> serde_json::Value {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn a_new_game_opens_on_the_title_with_its_hints() {
        let v = json(&Game::new(&sample(), &notes(), GLOSSARY).ui());
        assert_eq!(v["ok"]["screen"], "title");
        assert_eq!(v["ok"]["hints"][0]["term"], "Fold");
        assert_eq!(v["ok"]["hints"][0]["definition"], "Where the sheep go.");
    }

    #[test]
    fn a_bad_strings_file_comes_back_in_error() {
        let v = json(&Game::new("{}", &notes(), "").ui());
        assert_eq!(v["error"], "The strings file has no \"title\".");
    }

    #[test]
    fn a_bad_glossary_comes_back_in_error() {
        let v = json(&Game::new(&sample(), &notes(), "### Fold\n").ui());
        assert_eq!(v["error"], "The glossary entry \"Fold\" has no definition.");
    }

    #[test]
    fn act_goes_through_the_one_door() {
        let mut g = Game::new(&sample(), &notes(), "");
        let v = json(&g.act("menu.settings", ""));
        assert_eq!(v["ok"]["screen"], "settings");
    }

    #[test]
    fn act_refuses_an_unknown_action_in_error() {
        let mut g = Game::new(&sample(), &notes(), "");
        let v = json(&g.act("screen.jump", "start"));
        assert_eq!(v["error"], "Unknown action \"screen.jump\".");
    }
}
