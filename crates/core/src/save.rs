//! Saves that survive a deploy. Part of the saves kit (`kits/saves/README.md`).
//!
//! The core owns the save: what is in it, its version, and how a save from
//! an older build becomes one this build can use. The page stores the text
//! the core gives it (`web/save/store.js`) and hands it back; it never looks
//! inside. A save this build cannot read is refused and kept, never
//! overwritten, so a bad deploy costs a reload and not a player's progress.
//!
//! Every change to what a save holds bumps `VERSION` and adds one step to
//! `migrate`. `tests/old_saves.rs` loads a save written by every deployed
//! build (`tests/saves/`), and the project writes one more before each
//! deploy. A field an old save holds either reaches `Save` or is listed in
//! `DROPPED` with the reason, because the loader refuses fields it does not
//! know.
//!
//! Fleeced keeps the player's settings: the sound level, Dog mode and the
//! name for the lobby. The match settings belong to the host's lobby and are
//! not kept.

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

/// The shape this build writes. Bump it, and add a step to `migrate`,
/// whenever a field is added, renamed, moved or removed.
pub const VERSION: u32 = 1;

/// Fields an old save holds that a migration removed or changed on purpose,
/// each with the version that did it and why. Every other field comes back
/// from loading with the value the old save held.
pub const DROPPED: &[(&str, u32, &str)] = &[];

/// Status keys. Each has a string in `data/` under the same key.
pub const UNREADABLE: &str = "save.unreadable";
pub const NEWER: &str = "save.newer";
pub const NO_STEP: &str = "save.no_step";

/// The sound level a new player starts at, percent.
pub const DEFAULT_VOLUME: u32 = 50;

/// What a save holds. Unknown fields are refused, not dropped; a missing
/// field takes its default, which is why old_saves.rs checks that none is lost.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Save {
    pub version: u32,
    /// percent, 0 to 100
    pub volume: u32,
    pub dog_mode: bool,
    /// the player's name, empty until they type one
    pub name: String,
}

impl Default for Save {
    fn default() -> Save {
        Save {
            version: VERSION,
            volume: DEFAULT_VOLUME,
            dog_mode: false,
            name: String::new(),
        }
    }
}

/// What came back from the page's storage or a file the player chose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Loaded {
    /// Nothing was stored: start with the defaults.
    Fresh,
    /// A save this build can use, migrated to `VERSION`.
    Ready(Save),
    /// A save this build cannot use. `kept` is the text, unchanged, for the
    /// page to keep beside the new save and offer back as a file; `why` is a
    /// status key.
    Refused { why: &'static str, kept: String },
}

pub fn new_game() -> Save {
    Save::default()
}

/// The text the page stores.
pub fn store(save: &Save) -> String {
    let mut s = save.clone();
    s.version = VERSION;
    serde_json::to_string(&s).expect("a Save always serializes")
}

/// One step from a save of version `from` to `from + 1`. Pure: it reads the
/// old fields and writes the new ones. Version 1 is the first, so there is
/// no step yet; the first change to `Save` adds one `match` here.
fn migrate(_from: u32, _old: Map<String, Value>) -> Option<Map<String, Value>> {
    None
}

pub fn load(text: &str) -> Loaded {
    if text.trim().is_empty() {
        return Loaded::Fresh;
    }
    let refuse = |why| Loaded::Refused {
        why,
        kept: text.to_string(),
    };
    let Ok(Value::Object(mut v)) = serde_json::from_str::<Value>(text) else {
        return refuse(UNREADABLE);
    };
    let Some(mut version) = v.get("version").and_then(Value::as_u64) else {
        return refuse(UNREADABLE);
    };
    if version > VERSION as u64 {
        return refuse(NEWER);
    }
    while version < VERSION as u64 {
        match migrate(version as u32, v) {
            Some(next) => v = next,
            None => return refuse(NO_STEP),
        }
        version += 1;
        v.insert("version".into(), json!(version));
    }
    match serde_json::from_value::<Save>(Value::Object(v)) {
        Ok(save) => Loaded::Ready(save),
        Err(_) => refuse(UNREADABLE),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Save {
        Save {
            version: VERSION,
            volume: 35,
            dog_mode: true,
            name: "Morag".into(),
        }
    }

    #[test]
    fn a_save_reads_back_as_it_was_written() {
        assert_eq!(load(&store(&sample())), Loaded::Ready(sample()));
    }

    #[test]
    fn nothing_stored_is_a_new_game() {
        assert_eq!(load(""), Loaded::Fresh);
        assert_eq!(load("  \n"), Loaded::Fresh);
    }

    #[test]
    fn a_save_this_build_cannot_use_is_refused_and_kept_word_for_word() {
        for (text, why) in [
            ("{nope", UNREADABLE),
            ("[1,2]", UNREADABLE),
            (r#"{"volume":3}"#, UNREADABLE),
            (r#"{"version":99,"volume":3}"#, NEWER),
            (r#"{"version":0,"volume":3}"#, NO_STEP),
            (r#"{"version":1,"volume":3,"cheat":true}"#, UNREADABLE),
        ] {
            assert_eq!(
                load(text),
                Loaded::Refused {
                    why,
                    kept: text.to_string()
                },
                "{text}"
            );
        }
    }

    #[test]
    fn a_new_game_stores_at_this_version() {
        assert!(store(&new_game()).contains(&format!("\"version\":{VERSION}")));
    }
}
