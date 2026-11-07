//! The match's heads-up display, as page values: words from `data/` and whole
//! percents. It reads one viewer's `Snapshot`, which holds nothing that viewer
//! isn't to know (FL-7), so the page has nothing to leak.

use serde_json::{json, Value};

use crate::body::{ACT, DISCARD, PRIMARY, SECONDARY, SPRINT};
use crate::fx::{self, Fx};
use crate::game::{Snapshot, Target, KEYS};
use crate::icon;
use crate::rules::outcome::Outcome;
use crate::rules::Role;
use crate::scene::percent;
use crate::strings::Strings;

fn pair(strings: &Strings, key: &str, value: String) -> Value {
    json!({ "label": strings.get(key), "value": value })
}

/// Minutes and seconds.
pub fn clock(seconds: i32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// A key's name on its small cap, from the code `web/input.js` gives it.
fn key_name(strings: &Strings, code: &str) -> String {
    match code {
        "Mouse0" => return strings.get("key.left_click"),
        "Mouse2" => return strings.get("key.right_click"),
        _ if code.starts_with("Arrow") => return code.to_string(),
        _ => {}
    }
    let name = ["Key", "Digit"].iter().find_map(|p| code.strip_prefix(p));
    let side = ["Left", "Right"].iter().find_map(|s| code.strip_suffix(s));
    name.or(side).unwrap_or(code).to_string()
}

/// The cap for an action: the first key `table` gives it, the one the page
/// reads from `ui.keys`.
fn cap(strings: &Strings, table: &[(&str, u32)], action: u16) -> String {
    let key = table.iter().find(|(_, bit)| *bit == action as u32);
    key.map_or_else(String::new, |(code, _)| key_name(strings, code))
}

/// What the action key would do, for the line under the crosshair.
fn prompt(s: &Snapshot) -> Option<&'static str> {
    match s.me.target? {
        Target::Mound(_) if s.me.held.is_some() => None,
        Target::Mound(_) => Some("hud.dig"),
        Target::Mouth(_) => Some("hud.tunnel"),
    }
}

fn dial(strings: &Strings, s: &Snapshot) -> Value {
    let (key, fraction, seconds) = match s.me.role {
        Role::Wolf => ("hud.eat", s.me.eat_dial, s.me.eat_seconds),
        Role::Sheepdog => ("hud.lunge", s.me.lunge_dial, None),
    };
    let share = percent(fraction.unwrap_or(0));
    let value = match seconds {
        Some(n) if n > 0 => n.to_string(),
        _ if share < 100 => String::new(),
        _ => strings.get("hud.ready"),
    };
    json!({
        "label": strings.get(key),
        "key": cap(strings, KEYS, PRIMARY),
        "value": value,
        "percent": share,
        "ready": share >= 100,
    })
}

/// The HUD for one viewer.
pub fn hud(strings: &Strings, s: &Snapshot) -> Value {
    let mut counts = vec![pair(
        strings,
        "hud.in_fold",
        format!("{} / {}", s.view.in_fold, s.fold_target),
    )];
    counts.push(pair(
        strings,
        "hud.eats",
        format!("{} / {}", s.view.eats, s.eat_goal),
    ));
    let powerup = s.me.held_name.map(|name| {
        let id = s.me.held.unwrap_or("");
        json!({
            "icon": id,
            "image": icon::powerup_url(id),
            "name": name,
            "percent": percent(s.me.held_left),
            "use": { "label": strings.get("hud.use"), "key": cap(strings, KEYS, SECONDARY) },
            "drop": { "label": strings.get("hud.drop"), "key": cap(strings, KEYS, DISCARD) },
        })
    });
    let effects: Vec<Value> =
        s.me.running
            .iter()
            .map(|r| {
                json!({
                    "image": icon::powerup_url(r.icon),
                    "name": r.name,
                    "percent": percent(r.left),
                })
            })
            .collect();
    let message = if let Some(n) = s.me.back_in {
        Some(strings.fill("hud.back_in", &[("n", n.to_string())]))
    } else if !s.me.alive {
        Some(strings.get("hud.out"))
    } else if s.me.at_fold {
        Some(strings.get("hud.fold_shut"))
    } else if s.view.wolf_killed && !s.me.killed {
        Some(strings.get("hud.kill"))
    } else {
        None
    };
    json!({
        "role": s.me.role,
        "time": clock(s.seconds_left),
        "counts": counts,
        "stamina": {
            "label": strings.get("hud.stamina"),
            "key": cap(strings, KEYS, SPRINT),
            "percent": percent(s.me.stamina),
        },
        "dial": dial(strings, s),
        "powerup": powerup,
        "effects": effects,
        "scoped": s.me.scoped,
        "zoomies": s.me.running.iter().any(|r| r.icon == "zoomies"),
        "dig": percent(s.me.dig),
        "prompt": prompt(s).map(|key| strings.get(key)),
        "prompt_key": cap(strings, KEYS, ACT),
        "message": message,
        "kill": s.me.killed.then(|| strings.get("hud.kill")),
        "flash": percent(s.me.kill_flash),
        "click": strings.get("hud.click"),
        "on_target": s.me.prey.is_some() || s.me.belling.is_some() || s.me.target.is_some(),
    })
}

/// Where this viewer stands and what's in sight, as whole centimeters: what
/// the browser checks steer by. Only what the viewer's `Snapshot` holds.
pub fn seen(s: &Snapshot, aim: (u16, i16)) -> Value {
    let cm = |v: Fx| ((v as i64 * 100) >> fx::SHIFT) as i32;
    let me = s.bodies.iter().find(|b| b.seat == s.me.seat);
    let spot = |row: &[Fx; 2]| json!([cm(row[0]), cm(row[1])]);
    json!({
        "at": me.map(|b| [cm(b.x), cm(b.z)]),
        "aim": [aim.0, aim.1],
        "sheep": s.sheep.iter().map(|r| json!([r.id, cm(r.x), cm(r.z)])).collect::<Vec<_>>(),
        "mounds": s.mound_spots.iter().map(spot).collect::<Vec<_>>(),
        "prey": s.me.prey,
        "target": s.me.target,
        "bodies": s.bodies.iter().map(|b| json!([b.seat, cm(b.x), cm(b.z)])).collect::<Vec<_>>(),
    })
}

/// The heading and the line under it on the winner's screen.
pub fn result(strings: &Strings, outcome: Outcome) -> (String, String) {
    let heading = match outcome.winner() {
        Role::Sheepdog => "play.sheepdogs_win",
        Role::Wolf => "play.wolves_win",
    };
    let why = match outcome {
        Outcome::WolvesAte => "play.why_ate",
        Outcome::FoldFilled => "play.why_fold",
        Outcome::LastWolfDead => "play.why_last_wolf",
        Outcome::TimeRanOut => "play.why_time",
    };
    (strings.get(heading), strings.get(why))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::{from_int, ONE};
    use crate::game::Match;
    use crate::lobby::{MatchStart, Role as Pick, StartSeat};
    use crate::map::Map;
    use crate::rules::setup::Settings;
    use crate::strings::tests::sample;
    use std::sync::Arc;

    fn game() -> Match {
        let seat = |k: u32, role| StartSeat {
            seat: k,
            role,
            person: format!("p{k}"),
            name: format!("p{k}"),
        };
        let start = MatchStart {
            seed: 1,
            seats: vec![seat(0, Pick::Sheepdog), seat(1, Pick::Wolf)],
            settings: Settings {
                sheep: 20,
                eats_required: 5,
                ..Settings::default()
            },
        };
        Match::new(&start, Arc::new(Map::generate(1)))
    }

    fn words() -> Strings {
        Strings::parse(&sample()).unwrap()
    }

    #[test]
    fn a_key_s_cap_reads_as_the_key_the_table_gives_the_action() {
        let w = words();
        assert_eq!(cap(&w, KEYS, SPRINT), "Shift");
        assert_eq!(cap(&w, KEYS, ACT), "E");
        assert_eq!(cap(&w, KEYS, DISCARD), "F");
        assert_eq!(cap(&w, KEYS, PRIMARY), "key.left_click");
        assert_eq!(cap(&w, KEYS, SECONDARY), "key.right_click");
        // a table with the keys moved names the new ones
        let moved = [
            ("ControlRight", SPRINT as u32),
            ("Digit3", ACT as u32),
            ("KeyQ", DISCARD as u32),
            ("Mouse2", PRIMARY as u32),
            ("Space", SECONDARY as u32),
            ("ArrowLeft", 1 << 15),
        ];
        assert_eq!(cap(&w, &moved, SPRINT), "Control");
        assert_eq!(cap(&w, &moved, ACT), "3");
        assert_eq!(cap(&w, &moved, DISCARD), "Q");
        assert_eq!(cap(&w, &moved, PRIMARY), "key.right_click");
        assert_eq!(cap(&w, &moved, SECONDARY), "Space");
        assert_eq!(cap(&w, &moved, 1 << 15), "ArrowLeft");
        assert_eq!(cap(&w, &moved, 1 << 14), "");
    }

    #[test]
    fn each_key_sits_beside_what_it_drives() {
        let mut s = game().snapshot_for(1).unwrap();
        s.me.held = Some("bone");
        s.me.held_name = Some("Bone");
        s.me.held_left = ONE;
        s.me.target = Some(Target::Mouth(0));
        let h = hud(&words(), &s);
        assert_eq!(h["stamina"]["key"], "Shift");
        assert_eq!(h["dial"]["key"], "key.left_click");
        assert_eq!(h["powerup"]["use"]["key"], "key.right_click");
        assert_eq!(h["powerup"]["use"]["label"], "hud.use");
        assert_eq!(h["powerup"]["drop"]["key"], "F");
        assert_eq!(h["powerup"]["drop"]["label"], "hud.drop");
        assert_eq!(h["prompt"], "hud.tunnel");
        assert_eq!(h["prompt_key"], "E");
    }

    #[test]
    fn a_clock_shows_minutes_and_two_figures_of_seconds() {
        assert_eq!(clock(480), "8:00");
        assert_eq!(clock(65), "1:05");
        assert_eq!(clock(0), "0:00");
    }

    #[test]
    fn a_percent_is_a_whole_number_from_a_share() {
        assert_eq!(percent(ONE / 2), 50);
        assert_eq!(percent(ONE), 100);
        assert_eq!(percent(from_int(3)), 100);
        assert_eq!(percent(-ONE), 0);
    }

    #[test]
    fn both_teams_huds_count_the_eats() {
        let m = game();
        let dog = hud(&words(), &m.snapshot_for(0).unwrap());
        let wolf = hud(&words(), &m.snapshot_for(1).unwrap());
        let labels = |h: &Value| h["counts"].as_array().unwrap().len();
        assert_eq!(labels(&dog), 2);
        assert_eq!(labels(&wolf), 2);
        assert_eq!(dog["counts"][1]["value"], "0 / 5");
        assert_eq!(wolf["counts"][1]["value"], "0 / 5");
    }

    #[test]
    fn a_wolf_s_dial_counts_down_from_a_full_cooldown() {
        let m = game();
        let wolf = hud(&words(), &m.snapshot_for(1).unwrap());
        assert_eq!(wolf["dial"]["value"], "20");
        assert_eq!(wolf["dial"]["percent"], 0);
        assert_eq!(wolf["dial"]["ready"], false);
    }

    #[test]
    fn a_kill_is_a_banner_for_the_killer_and_a_line_for_the_other_sheepdogs() {
        let mut s = game().snapshot_for(0).unwrap();
        assert!(hud(&words(), &s)["kill"].is_null());
        s.view.wolf_killed = true;
        let line = hud(&words(), &s);
        assert_eq!(line["message"], "hud.kill");
        assert!(line["kill"].is_null());
        s.me.killed = true;
        s.me.kill_flash = ONE / 2;
        let banner = hud(&words(), &s);
        assert_eq!(banner["kill"], "hud.kill");
        assert!(banner["message"].is_null());
        assert_eq!(banner["flash"], 50);
    }

    #[test]
    fn a_killed_wolf_sees_a_countdown_until_it_is_back() {
        let mut s = game().snapshot_for(1).unwrap();
        s.me.alive = false;
        s.me.back_in = Some(23);
        assert_eq!(hud(&words(), &s)["message"], "hud.back_in");
        s.me.back_in = None;
        assert_eq!(hud(&words(), &s)["message"], "hud.out");
    }

    #[test]
    fn a_powerup_held_and_one_running_show_their_icons() {
        let mut s = game().snapshot_for(1).unwrap();
        assert!(hud(&words(), &s)["powerup"].is_null());
        s.me.held = Some("bone");
        s.me.held_name = Some("Bone");
        s.me.held_left = ONE;
        s.me.running = vec![crate::game::Running {
            icon: "sheep-suit",
            name: "Sheep suit",
            left: ONE / 4,
        }];
        let h = hud(&words(), &s);
        assert_eq!(h["powerup"]["image"], icon::powerup_url("bone"));
        assert_eq!(h["effects"][0]["image"], icon::powerup_url("sheep-suit"));
        assert_eq!(h["effects"][0]["percent"], 25);
    }

    #[test]
    fn zoomies_show_only_while_they_run() {
        let mut s = game().snapshot_for(1).unwrap();
        assert_eq!(hud(&words(), &s)["zoomies"], false);
        s.me.running = vec![crate::game::Running {
            icon: "sheep-suit",
            name: "Sheep suit",
            left: ONE,
        }];
        assert_eq!(hud(&words(), &s)["zoomies"], false);
        s.me.running[0].icon = "zoomies";
        assert_eq!(hud(&words(), &s)["zoomies"], true);
    }

    #[test]
    fn every_way_a_match_ends_has_its_own_line() {
        let w = words();
        let (head, why) = result(&w, Outcome::WolvesAte);
        assert_eq!(head, w.get("play.wolves_win"));
        assert_eq!(why, w.get("play.why_ate"));
        let lines: Vec<String> = [
            Outcome::WolvesAte,
            Outcome::FoldFilled,
            Outcome::LastWolfDead,
            Outcome::TimeRanOut,
        ]
        .iter()
        .map(|o| result(&w, *o).1)
        .collect();
        for (k, a) in lines.iter().enumerate() {
            assert!(!lines[k + 1..].contains(a), "{a}");
        }
    }
}
