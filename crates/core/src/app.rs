//! The app: which screen shows, the player's settings, and what every action
//! does. `api.rs` holds one `App` for the page and forwards to it; `ui` is
//! everything the page draws. No clock and no randomness: the page hands in
//! a seed and the time. After Crabble's `app.rs`.

mod lobby_view;

use std::collections::BTreeMap;

use crate::dock::{self, Dock};
use crate::game;
use crate::hud;
use crate::lobby::{clean_name, Role, Who, MAX_SEATS};
use crate::online::room::{clean_code, Kind, Opening, Pump, Request, Room, Stage};
use crate::online::APP_ID;
use crate::rng::Rng;
use crate::save::{self, Loaded, Save};
use crate::scene::{Scene, Slot};
use crate::strings::Strings;
use crate::view::{Hint, Item, LineupSeat, NoteEntry, Panel, Ui};
use crate::{glossary, icon, patch_notes};

/// The build's version: the version button's text, and the top row of
/// `data/patch-notes.json`. `scripts/bump.sh` changes the two together.
pub const VERSION: &str = "0.2.0";

/// The sound slider's step, percent.
const VOLUME_STEP: u32 = 5;
/// The sound slider's top, percent.
const VOLUME_MAX: u32 = 100;
/// How long the last moment of a match holds before the winners' screen,
/// milliseconds: the ending lands and the winners' tune starts.
const WIN_HOLD_MS: i64 = 1800;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Title,
    Settings,
    Notes,
    Online,
    /// A joiner's pasted-code form, until the host's lobby is heard.
    Paste,
    Lobby,
    Play,
    /// A match has ended: the winning team on the hillside.
    Won,
}

impl Screen {
    fn name(self) -> &'static str {
        match self {
            Screen::Title => "title",
            Screen::Settings => "settings",
            Screen::Notes => "notes",
            Screen::Online => "online",
            Screen::Paste => "paste",
            Screen::Lobby => "lobby",
            Screen::Play => "play",
            Screen::Won => "won",
        }
    }
}

/// The page's one app.
#[derive(Debug, Clone)]
pub struct App {
    strings: Strings,
    notes: Vec<patch_notes::Entry>,
    hints: Vec<Hint>,
    screen: Screen,
    /// Where Back goes from Settings and What's New.
    back: Screen,
    save: Save,
    /// A save this build refused, until the page has taken it.
    keep_refused: String,
    /// The status key that says why, for as long as Settings can offer it.
    refused_why: &'static str,
    /// Settings' button asked for the refused save as a file.
    offer_old_file: bool,
    /// `?fast`: the browser checks skip what only makes a person wait.
    fast: bool,
    /// `?codes`: show the invite-code buttons on the Online screen.
    codes: bool,
    /// `?net=off`: rooms open with no transport, for the checks.
    no_net: bool,
    /// This browser's id, stable for its tab, from the page.
    client: String,
    /// The build's stamp: a different one is turned away at the door.
    build: String,
    /// The page's own address, for the invite link.
    page_url: String,
    rng: Rng,
    /// The room this browser is in.
    room: Option<Room>,
    /// The match's scene and the seed it was built for: the page opens it.
    scene: Slot,
    /// A room just left: the transport still has to hear it.
    leaving: Vec<Request>,
    /// The Online screen's code field, and whether Join refused it.
    typed_code: String,
    bad_code: bool,
    /// The pasted-code forms' fields: a joiner's invite, a host's reply.
    typed_invite: String,
    typed_reply: String,
    /// The latest time the page handed in.
    now_ms: i64,
    /// When the ended match gives way to the winners' screen.
    win_at: Option<i64>,
    /// The window, in CSS pixels.
    width: u32,
    height: u32,
}

fn on_off(value: &str) -> Result<bool, String> {
    match value {
        "on" => Ok(true),
        "off" => Ok(false),
        other => Err(format!("\"{other}\" is neither on nor off.")),
    }
}

impl App {
    /// Checks the strings file, the patch notes and the glossary and opens
    /// on the Title.
    pub fn new(strings_json: &str, notes_json: &str, glossary_md: &str) -> Result<App, String> {
        let strings = Strings::parse(strings_json).map_err(|e| e.message())?;
        let notes = patch_notes::parse(notes_json)?;
        if notes[0].version != VERSION {
            return Err(format!(
                "The patch notes start at {}, and this build is {VERSION}.",
                notes[0].version
            ));
        }
        let hints = glossary::parse(glossary_md)
            .map_err(|e| e.message())?
            .into_iter()
            .map(|e| Hint {
                term: e.term,
                definition: e.definition,
            })
            .collect();
        Ok(App {
            strings,
            notes,
            hints,
            screen: Screen::Title,
            back: Screen::Title,
            save: save::new_game(),
            keep_refused: String::new(),
            refused_why: "",
            offer_old_file: false,
            fast: false,
            codes: false,
            no_net: false,
            client: String::new(),
            build: String::new(),
            page_url: String::new(),
            rng: Rng::new(0),
            room: None,
            scene: Slot::default(),
            leaving: Vec::new(),
            typed_code: String::new(),
            bad_code: false,
            typed_invite: String::new(),
            typed_reply: String::new(),
            now_ms: 0,
            win_at: None,
            width: 0,
            height: 0,
        })
    }

    /// Opens a screen that Back returns from.
    fn open(&mut self, screen: Screen) {
        self.back = self.screen;
        self.screen = screen;
    }

    /// The stored text, read once as the page loads.
    fn load_save(&mut self, text: &str) {
        match save::load(text) {
            Loaded::Fresh => {}
            Loaded::Ready(s) => self.save = s,
            Loaded::Refused { why, kept } => {
                self.keep_refused = kept;
                self.refused_why = why;
            }
        }
    }

    /// What a new room needs from the page.
    fn opening(&mut self) -> Opening {
        Opening {
            client: self.client.clone(),
            name: self.save.name.clone(),
            build: self.build.clone(),
            app_id: APP_ID.to_string(),
            seed: self.rng.next_u64(),
            fast: self.fast,
            no_net: self.no_net,
        }
    }

    fn enter(&mut self, room: Room) {
        self.room = Some(room);
        self.bad_code = false;
        self.back = Screen::Title;
        self.screen = Screen::Lobby;
    }

    /// Joins by a typed or linked code, or says it is wrong.
    fn join(&mut self, code: &str) {
        match clean_code(code) {
            Some(code) => {
                let how = self.opening();
                self.enter(Room::join(&how, &code, self.now_ms));
            }
            None => {
                self.typed_code = code.to_string();
                self.bad_code = true;
                if self.screen != Screen::Online {
                    self.open(Screen::Online);
                }
            }
        }
    }

    fn leave(&mut self) {
        if self.room.take().is_some_and(|r| r.kind != Kind::Solo) {
            self.leaving.push(Request::Leave);
        }
        self.typed_invite.clear();
        self.typed_reply.clear();
        self.back = Screen::Title;
        self.screen = Screen::Title;
    }

    /// Applies one named action. A refused action leaves the app as it was.
    pub fn act(&mut self, name: &str, value: &str) -> Result<(), String> {
        match name {
            "menu.solo" => {
                let how = self.opening();
                self.enter(Room::solo(&how));
            }
            "menu.online" => self.open(Screen::Online),
            "menu.settings" => self.open(Screen::Settings),
            "menu.back" => self.screen = self.back,
            "version" => {
                if self.screen != Screen::Notes {
                    self.open(Screen::Notes);
                }
            }
            "settings.volume" => {
                let v: u32 = value
                    .parse()
                    .map_err(|_| format!("\"{value}\" is not a sound level."))?;
                self.save.volume = v.min(VOLUME_MAX);
            }
            "settings.dog_mode" => self.save.dog_mode = on_off(value)?,
            "save.load" => self.load_save(value),
            // the page has the refused text now, and Settings still offers it
            "save.kept" => self.keep_refused.clear(),
            "save.old_file" => self.offer_old_file = true,
            "save.offered" => self.offer_old_file = false,
            "online.code" => {
                self.typed_code = value.to_string();
                self.bad_code = false;
            }
            "online.host" => {
                let how = self.opening();
                self.enter(Room::host(&how, self.now_ms));
            }
            "online.join" => self.join(&self.typed_code.clone()),
            "online.paste_host" => {
                let how = self.opening();
                self.enter(Room::host_by_code(&how));
            }
            "online.paste_join" => self.open(Screen::Paste),
            "online.offer_in" => self.typed_invite = value.to_string(),
            // the form stays up, so a refused invite can be pasted again
            "online.make_answer" => {
                let how = self.opening();
                self.room = Some(Room::join_by_code(&how, &self.typed_invite));
            }
            "online.answer_in" => self.typed_reply = value.to_string(),
            "online.accept_answer" => {
                if let Some(room) = &mut self.room {
                    room.accept_reply(&self.typed_reply);
                }
                self.typed_reply.clear();
            }
            "online.paste_back" => {
                self.leave();
                self.screen = Screen::Online;
            }
            "lobby.leave" | "play.leave" => self.leave(),
            // for the checks and the shots: a solo match ends on its next tick
            "flag.end" => {
                let solo = self.room.as_mut().filter(|r| r.kind == Kind::Solo);
                if let Some(game) = solo.and_then(|r| r.game.as_mut()) {
                    match value {
                        "wolf" => game.goals.eats_required = 0,
                        _ => game.goals.match_ticks = 1,
                    }
                }
            }
            "flag.fast" => self.fast = true,
            "flag.codes" => self.codes = true,
            "flag.net_off" => self.no_net = true,
            "flag.client" => self.client = value.to_string(),
            "flag.build" => self.build = value.to_string(),
            "flag.page_url" => self.page_url = value.to_string(),
            "flag.seed" => {
                let seed: u64 = value.parse().map_err(|_| format!("\"{value}\" is not a seed."))?;
                self.rng = Rng::new(seed);
            }
            "flag.room" => self.join(value),
            "view.size" => {
                let size = value.split_once(' ');
                let parsed = size.and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)));
                (self.width, self.height) = parsed.ok_or_else(|| format!("\"{value}\" is not a size."))?;
            }
            other => {
                let Some(room) = &mut self.room else {
                    return Err(format!("Unknown action \"{other}\"."));
                };
                room.act(other, value)?;
                if other == "lobby.name" {
                    self.save.name = clean_name(value);
                }
            }
        }
        Ok(())
    }

    /// One event from the transport. With no room open there is no one to
    /// hear it: a room just left can still send a last word.
    pub fn net_event(&mut self, kind: &str, peer: &str, text: &str, host: bool, now_ms: i64) {
        if let Some(room) = &mut self.room {
            room.event(kind, peer, text, host, now_ms);
        }
    }

    /// One display frame: the room's work, and the screen following it.
    pub fn pump(&mut self, now_ms: i64, keys: u32) -> Pump {
        self.now_ms = now_ms;
        let mut pump = self
            .room
            .as_mut()
            .map_or_else(Pump::default, |r| r.pump(now_ms, keys));
        pump.requests.append(&mut self.leaving);
        if self.screen == Screen::Paste && self.room.as_ref().is_some_and(|r| r.heard) {
            self.screen = Screen::Lobby;
        }
        let started = self.room.as_ref().is_some_and(|r| r.stage != Stage::Lobby);
        if started && self.screen == Screen::Lobby {
            self.screen = Screen::Play;
        }
        if !started && matches!(self.screen, Screen::Play | Screen::Won) {
            self.screen = Screen::Lobby;
        }
        let over = self.room.as_ref().is_some_and(Room::over);
        if !over {
            self.win_at = None;
            if self.screen == Screen::Won {
                self.screen = Screen::Play;
            }
        } else if self.screen == Screen::Play {
            let hold = if self.fast { 0 } else { WIN_HOLD_MS };
            if now_ms >= *self.win_at.get_or_insert(now_ms + hold) {
                self.screen = Screen::Won;
            }
        }
        pump
    }

    fn button(&self, id: &str, key: &str) -> Item {
        Item::Button {
            id: id.into(),
            text: self.strings.get(key),
            quiet: false,
            disabled: false,
        }
    }

    fn quiet(&self, id: &str, key: &str) -> Item {
        Item::Button {
            id: id.into(),
            text: self.strings.get(key),
            quiet: true,
            disabled: false,
        }
    }

    fn title_panels(&self) -> Vec<Panel> {
        vec![Panel::new(
            "title",
            self.strings.get("title"),
            vec![
                self.button("menu.solo", "menu.solo"),
                self.button("menu.online", "menu.online"),
                self.quiet("menu.settings", "menu.settings"),
            ],
        )]
    }

    fn settings_panels(&self) -> Vec<Panel> {
        let mut items = vec![
            Item::Slider {
                id: "settings.volume".into(),
                label: self.strings.get("settings.sound"),
                min: 0,
                max: VOLUME_MAX,
                step: VOLUME_STEP,
                value: self.save.volume,
                text: format!("{}%", self.save.volume),
            },
            Item::Toggle {
                id: "settings.dog_mode".into(),
                label: self.strings.get("settings.dog_mode"),
                value: self.save.dog_mode,
                disabled: false,
            },
        ];
        if !self.refused_why.is_empty() {
            items.push(Item::Lines {
                id: String::new(),
                lines: vec![self.strings.get(self.refused_why)],
            });
            items.push(self.quiet("save.old_file", "save.old_file"));
        }
        items.push(self.quiet("menu.back", "menu.back"));
        vec![Panel::new(
            "settings",
            self.strings.get("settings.heading"),
            items,
        )]
    }

    fn notes_panels(&self) -> Vec<Panel> {
        let entries = self
            .notes
            .iter()
            .map(|e| NoteEntry {
                version: e.version.clone(),
                date: e.date.clone(),
                minor: e.minor(),
                lines: e.notes.clone(),
            })
            .collect();
        vec![Panel::new(
            "notes",
            self.strings.get("notes.heading"),
            vec![
                Item::Notes {
                    id: "notes.list".into(),
                    entries,
                },
                self.quiet("menu.back", "menu.back"),
            ],
        )]
    }

    fn online_panels(&self) -> Vec<Panel> {
        vec![Panel::new(
            "online",
            self.strings.get("online.heading"),
            vec![
                self.button("online.host", "online.host"),
                Item::Text {
                    id: "online.code".into(),
                    label: self.strings.get("online.code"),
                    value: self.typed_code.clone(),
                    max: crate::online::room::CODE_LEN as u32,
                    placeholder: String::new(),
                },
                self.button("online.join", "online.join"),
            ]
            .into_iter()
            .chain(
                self.codes
                    .then(|| self.quiet("online.paste_host", "online.paste_host")),
            )
            .chain(
                self.codes
                    .then(|| self.quiet("online.paste_join", "online.paste_join")),
            )
            .chain([self.quiet("menu.back", "menu.back")])
            .collect(),
        )]
    }

    /// The mouse moved in a match.
    pub fn look(&mut self, dx: i32, dy: i32) {
        if let Some(room) = &mut self.room {
            room.look(dx, dy);
        }
    }

    /// The mouse wheel turned in a match, as the page's delta.
    pub fn wheel(&mut self, delta: i32) {
        if let Some(room) = &mut self.room {
            room.wheel(delta);
        }
    }

    /// Builds the match's scene, if there's a match and it has none yet.
    pub fn open_scene(&mut self) -> Option<&Scene> {
        let room = self.room.as_ref()?;
        let (game, start) = (room.game.as_ref()?, room.start.as_ref()?);
        if self.scene.0.as_ref().is_none_or(|s| s.0 != start.seed) {
            let built = Scene::new(game, start.seed, room.session.seat);
            self.scene = Slot(Some((start.seed, built)));
        }
        self.scene()
    }

    pub fn close_scene(&mut self) {
        self.scene = Slot(None);
    }

    pub fn scene(&self) -> Option<&Scene> {
        self.scene.0.as_ref().map(|s| &s.1)
    }

    pub fn resize_scene(&mut self, css_w: i32, css_h: i32, dpr_pct: i32) -> [i32; 2] {
        match &mut self.scene.0 {
            Some((_, scene)) => scene.resize(css_w, css_h, dpr_pct),
            None => [0, 0],
        }
    }

    /// This frame's scene, from the newest of the match and the look.
    pub fn build_scene(&mut self) {
        let room = self.room.as_ref();
        let game = room.and_then(|r| r.game.as_ref());
        let names = room.map(|r| (0..MAX_SEATS).map(|n| self.seat_name(r, n)).collect());
        if let (Some(room), Some(game), Some((_, scene))) = (room, game, &mut self.scene.0) {
            scene.set_names(names.unwrap_or_default());
            scene.build(game, room.aim, room.zoom);
        }
    }

    /// The HUD for this browser's seat, or null with no match.
    pub fn hud(&self) -> serde_json::Value {
        let room = self.room.as_ref();
        let game = room.and_then(|r| Some((r.session.seat, r.game.as_ref()?)));
        let shown = game.and_then(|(seat, g)| g.snapshot_for(seat));
        shown.map_or(serde_json::Value::Null, |s| hud::hud(&self.strings, &s))
    }

    /// Where this viewer stands and what's in sight, in centimeters: what the
    /// browser checks steer by. It comes from the same `Snapshot` as the HUD.
    pub fn seen(&self) -> serde_json::Value {
        let room = self.room.as_ref();
        let game = room.and_then(|r| Some((r.session.seat, r.game.as_ref()?, r.aim)));
        game.and_then(|(seat, g, aim)| Some((g.snapshot_for(seat)?, aim)))
            .map_or(serde_json::Value::Null, |(s, aim)| hud::seen(&s, aim))
    }

    /// Everything the page draws now.
    pub fn ui(&self) -> Ui {
        let room = self.room.as_ref();
        let panels = match (self.screen, room) {
            (Screen::Settings, _) => self.settings_panels(),
            (Screen::Notes, _) => self.notes_panels(),
            (Screen::Online, _) => self.online_panels(),
            (Screen::Paste, r) => self.paste_panels(r),
            (Screen::Lobby, Some(r)) => self.lobby_panels(r),
            (Screen::Play, Some(r)) => self.play_panels(r),
            (Screen::Won, Some(r)) => self.won_panels(r),
            (Screen::Title | Screen::Lobby | Screen::Play | Screen::Won, _) => self.title_panels(),
        };
        let in_lobby = self.screen == Screen::Lobby && room.is_some_and(|r| r.heard);
        let won = self.screen == Screen::Won && room.is_some();
        let playing =
            self.screen == Screen::Play && room.is_some_and(|r| r.stage == Stage::Playing && !r.over());
        let status = match room {
            Some(r) => r.status(),
            None if self.bad_code => "online.bad_code",
            None => "",
        };
        let mut keys = BTreeMap::new();
        if playing {
            keys.extend(game::KEYS.iter().map(|(code, bit)| (code.to_string(), *bit)));
        }
        Ui {
            screen: self.screen.name().into(),
            title: self.strings.get("title"),
            version: VERSION.into(),
            version_opens: !matches!(self.screen, Screen::Notes | Screen::Play | Screen::Won),
            panels,
            hints: self.hints.clone(),
            icon: icon::data_url(),
            save: save::store(&self.save),
            keep_refused: self.keep_refused.clone(),
            offer_old_file: self.offer_old_file,
            volume: self.save.volume,
            dog_mode: self.save.dog_mode,
            live: room.is_some(),
            capture: playing,
            keys,
            scene: match (self.screen, room.and_then(|r| r.start.as_ref())) {
                (Screen::Play, Some(start)) => start.seed.to_string(),
                _ => String::new(),
            },
            hash: room
                .filter(|r| !r.code.is_empty())
                .map_or_else(String::new, |r| format!("#room={}", r.code)),
            status: if status.is_empty() {
                String::new()
            } else {
                self.strings.get(status)
            },
            dock: if in_lobby || won {
                dock::lobby(self.width, self.height)
            } else {
                Dock::Center
            },
            lineup: match room {
                Some(r) if in_lobby => self.lineup(r),
                Some(r) if won => self.winners(r),
                _ => Vec::new(),
            },
            cheer: won,
        }
    }

    /// Every seated character, for the 3D lineup behind the docked panels.
    fn lineup(&self, room: &Room) -> Vec<LineupSeat> {
        let me = room.lobby.seat_of(&room.client);
        (0..MAX_SEATS)
            .filter(|n| room.lobby.seats[*n].who != Who::Empty)
            .map(|n| LineupSeat {
                seat: n as u32,
                look: room.lobby.seats[n].role.key().to_string(),
                name: self.seat_name(room, n),
                me: me == Some(n),
            })
            .collect()
    }

    /// The winning team's seats, bots too, as the match settled them.
    fn winners(&self, room: &Room) -> Vec<LineupSeat> {
        let outcome = room.game.as_ref().and_then(game::Match::outcome);
        let (Some(outcome), Some(start)) = (outcome, room.start.as_ref()) else {
            return Vec::new();
        };
        let team = Role::from(outcome.winner());
        start
            .seats
            .iter()
            .filter(|s| s.role == team)
            .map(|s| LineupSeat {
                seat: s.seat,
                look: team.key().to_string(),
                name: self.seat_name(room, s.seat as usize),
                me: !s.person.is_empty() && s.person == room.client,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
