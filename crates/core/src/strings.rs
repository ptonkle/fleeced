//! The reader-visible strings, by key, from `data/strings.json`. The core
//! lists every key it uses in `KEYS`, and the file must hold exactly those:
//! a missing key would draw as nothing, and a key no screen uses is a string
//! nobody reads. Keys starting with `_` are notes for people and are skipped.
//! A status key from the online or saves kits is a key here like any other.
//!
//! After Crabble's `strings.rs`.

use std::collections::BTreeMap;

/// Every key a screen reads, in the order the screens use them.
pub const KEYS: &[&str] = &[
    "title",
    "menu.solo",
    "menu.online",
    "menu.settings",
    "menu.back",
    "settings.heading",
    "settings.sound",
    "settings.dog_mode",
    "notes.heading",
    "save.unreadable",
    "save.newer",
    "save.no_step",
    "save.old_file",
    "online.heading",
    "online.host",
    "online.join",
    "online.code",
    "online.bad_code",
    "online.finding",
    "online.waiting",
    "online.playing",
    "online.reload",
    "online.garbled",
    "online.left",
    "online.full",
    "online.started",
    "online.desync",
    "online.timeout",
    "online.no_bundle",
    "online.no_relay",
    "online.no_path",
    "online.signal_failed",
    "online.paste_host",
    "online.paste_join",
    "online.invite_code",
    "online.reply_code",
    "online.make_reply",
    "online.connect",
    "online.send_reply",
    "online.wrong_code",
    "online.bad_paste",
    "lobby.heading",
    "lobby.solo_heading",
    "lobby.code",
    "lobby.copy_code",
    "lobby.copy_link",
    "lobby.name",
    "lobby.name_hint",
    "lobby.ready",
    "lobby.not_ready",
    "lobby.start",
    "lobby.leave",
    "lobby.need_both",
    "lobby.need_ready",
    "lobby.seats",
    "lobby.match",
    "lobby.minutes",
    "lobby.sheep",
    "lobby.eats",
    "lobby.powerups",
    "lobby.sudden",
    "lobby.defaults",
    "lobby.unknown",
    "lobby.map_size",
    "map_size.small",
    "map_size.medium",
    "map_size.big",
    "seat.empty",
    "seat.default_name",
    "seat.bot",
    "seat.host",
    "seat.you",
    "seat.is_ready",
    "seat.add_bot",
    "seat.remove_bot",
    "role.sheepdog",
    "role.wolf",
    "role.random",
    "play.heading",
    "play.leave",
    "play.starting",
    "play.sheepdogs_win",
    "play.wolves_win",
    "play.why_ate",
    "play.why_fold",
    "play.why_last_wolf",
    "play.why_time",
    "play.lobby",
    "hud.in_fold",
    "hud.eats",
    "hud.stamina",
    "hud.eat",
    "hud.lunge",
    "hud.ready",
    "hud.dig",
    "hud.tunnel",
    "hud.use",
    "hud.drop",
    "key.left_click",
    "key.right_click",
    "hud.out",
    "hud.back_in",
    "hud.kill",
    "hud.fold_shut",
    "hud.click",
];

/// The strings file, checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Strings {
    map: BTreeMap<String, String>,
}

/// Why a strings file cannot be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StringsError {
    NotJson(String),
    Empty(String),
    Missing(String),
    Unknown(String),
}

impl StringsError {
    /// The message the page shows. Wording that depends on the rule stays
    /// beside the rule.
    pub fn message(&self) -> String {
        match self {
            StringsError::NotJson(detail) => {
                format!("The strings file could not be read: {detail}")
            }
            StringsError::Empty(key) => format!("The strings file has an empty \"{key}\"."),
            StringsError::Missing(key) => format!("The strings file has no \"{key}\"."),
            StringsError::Unknown(key) => format!("The strings file has \"{key}\", which no screen reads."),
        }
    }
}

impl Strings {
    /// Parses and checks the strings file. A value that is only whitespace
    /// counts as empty, because the page would draw nothing.
    pub fn parse(json: &str) -> Result<Strings, StringsError> {
        let raw: BTreeMap<String, String> =
            serde_json::from_str(json).map_err(|e| StringsError::NotJson(e.to_string()))?;
        let map: BTreeMap<String, String> = raw.into_iter().filter(|(k, _)| !k.starts_with('_')).collect();
        for key in KEYS {
            match map.get(*key) {
                None => return Err(StringsError::Missing((*key).to_string())),
                Some(v) if v.trim().is_empty() => return Err(StringsError::Empty((*key).to_string())),
                Some(_) => {}
            }
        }
        if let Some(extra) = map.keys().find(|k| !KEYS.contains(&k.as_str())) {
            return Err(StringsError::Unknown(extra.clone()));
        }
        Ok(Strings { map })
    }

    /// The string for a key, or the key itself: a status key that reached
    /// the wrong list still draws as something a person can report.
    pub fn get(&self, key: &str) -> String {
        self.map.get(key).cloned().unwrap_or_else(|| key.to_string())
    }

    /// The string for a key with each `{name}` filled in.
    pub fn fill(&self, key: &str, values: &[(&str, String)]) -> String {
        let mut out = self.get(key);
        for (name, value) in values {
            out = out.replace(&format!("{{{name}}}"), value);
        }
        out
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A complete file for tests: each key's text is the key.
    pub(crate) fn sample() -> String {
        let map: BTreeMap<&str, &str> = KEYS.iter().map(|k| (*k, *k)).collect();
        serde_json::to_string(&map).unwrap()
    }

    #[test]
    fn a_complete_file_is_accepted() {
        let s = Strings::parse(&sample()).unwrap();
        assert_eq!(s.get("title"), "title");
    }

    #[test]
    fn an_empty_title_is_rejected_by_name() {
        let mut map: BTreeMap<String, String> = serde_json::from_str(&sample()).unwrap();
        map.insert("title".into(), "  ".into());
        let e = Strings::parse(&serde_json::to_string(&map).unwrap()).unwrap_err();
        assert_eq!(e, StringsError::Empty("title".into()));
        assert_eq!(e.message(), "The strings file has an empty \"title\".");
    }

    #[test]
    fn a_missing_key_is_rejected_by_name() {
        let mut map: BTreeMap<String, String> = serde_json::from_str(&sample()).unwrap();
        map.remove("menu.back");
        let e = Strings::parse(&serde_json::to_string(&map).unwrap()).unwrap_err();
        assert_eq!(e.message(), "The strings file has no \"menu.back\".");
    }

    #[test]
    fn an_unknown_key_is_rejected_and_a_note_for_people_is_not() {
        let mut map: BTreeMap<String, String> = serde_json::from_str(&sample()).unwrap();
        map.insert("_about".into(), "A note.".into());
        assert!(Strings::parse(&serde_json::to_string(&map).unwrap()).is_ok());
        map.insert("extra".into(), "x".into());
        let e = Strings::parse(&serde_json::to_string(&map).unwrap()).unwrap_err();
        assert_eq!(e, StringsError::Unknown("extra".into()));
    }

    #[test]
    fn text_that_is_not_json_is_rejected() {
        assert!(matches!(
            Strings::parse("title: T"),
            Err(StringsError::NotJson(_))
        ));
    }

    #[test]
    fn a_placeholder_is_filled_by_name() {
        let mut map: BTreeMap<String, String> = serde_json::from_str(&sample()).unwrap();
        map.insert("play.starting".into(), "Tick {n} of {m}".into());
        let s = Strings::parse(&serde_json::to_string(&map).unwrap()).unwrap();
        assert_eq!(
            s.fill("play.starting", &[("n", "3".into()), ("m", "9".into())]),
            "Tick 3 of 9"
        );
    }
}
