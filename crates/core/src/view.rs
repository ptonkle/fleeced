//! The view model's pieces: panels and the items inside them. The page has
//! one builder per item `type` and draws a panel without knowing what it
//! means; `kind` names the panel for the stylesheet only. After Crabble's
//! `view.rs`.
//!
//! Every control names an action (`id`) that the page sends back through
//! `act` with the control's new value: a button sends "", a toggle "on" or
//! "off", a slider, a number, a select or a text field the value it shows.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::dock::Dock;

fn is_false(b: &bool) -> bool {
    !*b
}

/// One block of the screen. Empty fields are left out of the JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Panel {
    /// the panel's name for the stylesheet ("title", "settings")
    pub kind: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub heading: String,
    /// a line under the heading
    #[serde(skip_serializing_if = "String::is_empty")]
    pub sub: String,
    pub items: Vec<Item>,
}

impl Panel {
    pub fn new(kind: &str, heading: String, items: Vec<Item>) -> Panel {
        Panel {
            kind: kind.into(),
            heading,
            sub: String::new(),
            items,
        }
    }
}

/// One choice of a select.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Choice {
    pub value: String,
    pub text: String,
}

/// One version on the What's New screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NoteEntry {
    pub version: String,
    pub date: String,
    pub minor: bool,
    pub lines: Vec<String>,
}

/// One item in a panel, by `type`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Item {
    Button {
        id: String,
        text: String,
        /// drawn smaller, for a less used action
        #[serde(skip_serializing_if = "is_false")]
        quiet: bool,
        #[serde(skip_serializing_if = "is_false")]
        disabled: bool,
    },
    Slider {
        id: String,
        label: String,
        min: u32,
        max: u32,
        step: u32,
        value: u32,
        /// the value as the reader reads it ("80%")
        text: String,
    },
    Toggle {
        id: String,
        label: String,
        value: bool,
        #[serde(skip_serializing_if = "is_false")]
        disabled: bool,
    },
    Select {
        id: String,
        label: String,
        value: String,
        options: Vec<Choice>,
        #[serde(skip_serializing_if = "is_false")]
        disabled: bool,
    },
    /// a one-line text field; the page sends its text when it changes
    Text {
        id: String,
        label: String,
        value: String,
        /// the most characters it takes
        max: u32,
        /// shown grayed while the field is empty
        #[serde(skip_serializing_if = "String::is_empty")]
        placeholder: String,
    },
    /// a whole number the page sends when it changes; the core checks it
    Number {
        id: String,
        label: String,
        value: u32,
        min: u32,
        max: u32,
        #[serde(skip_serializing_if = "is_false")]
        disabled: bool,
    },
    /// short lines of text, one after another; `id`, when not empty, is
    /// for the checks to find them
    Lines {
        #[serde(skip_serializing_if = "String::is_empty")]
        id: String,
        lines: Vec<String>,
    },
    Notes {
        id: String,
        entries: Vec<NoteEntry>,
    },
    /// a Copy button: the page puts `copy` on the clipboard when it is
    /// pressed; `value`, when not empty, is shown big beside it under
    /// `label` (the room code)
    Copy {
        id: String,
        label: String,
        value: String,
        copy: String,
        /// the button's text
        text: String,
    },
    /// a box of a long pasted code: a field the page sends on every change,
    /// or, `read_only`, the code to copy out
    CodeBox {
        id: String,
        label: String,
        value: String,
        #[serde(skip_serializing_if = "is_false")]
        read_only: bool,
    },
    /// one seat of the lobby: who sits there, and the controls this browser
    /// may use on it. The characters themselves turn in the 3D lineup behind
    /// the panels (`Ui::lineup`)
    Seat {
        id: String,
        /// "sheepdog", "wolf", "random", or "empty" with nobody seated: the
        /// stylesheet's key; `role` says the same in words
        look: String,
        name: String,
        /// who this is to the reader, and Ready: "You \u{b7} Ready", "Host", ""
        note: String,
        /// this browser's own seat
        #[serde(skip_serializing_if = "is_false")]
        me: bool,
        #[serde(skip_serializing_if = "is_false")]
        ready: bool,
        /// the role in words, shown when `controls` has no role select
        #[serde(skip_serializing_if = "String::is_empty")]
        role: String,
        controls: Vec<Item>,
    },
}

/// One character in the lobby's lineup, for the 3D view behind the docked
/// panels. The current player's has `me` set, for the lineup to mark clearly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineupSeat {
    /// the seat's number from 0
    pub seat: u32,
    /// "sheepdog", "wolf" or "random"
    pub look: String,
    pub name: String,
    pub me: bool,
}

/// A term and its definition, for the page's hover hints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Hint {
    pub term: String,
    pub definition: String,
}

/// Everything the page draws. The page keeps none of it between calls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ui {
    pub screen: String,
    pub title: String,
    /// the version button's text
    pub version: String,
    /// the version button opens What's New (not during a match)
    pub version_opens: bool,
    pub panels: Vec<Panel>,
    pub hints: Vec<Hint>,
    /// the tab icon, as a data URL
    pub icon: String,
    /// the save text for the page's store; it writes when this changes
    pub save: String,
    /// a save this build refused, for the page to keep beside the new one
    #[serde(skip_serializing_if = "String::is_empty")]
    pub keep_refused: String,
    /// the page offers the refused save as a file, then sends `save.offered`
    #[serde(skip_serializing_if = "is_false")]
    pub offer_old_file: bool,
    /// sound level, percent (web/audio.js)
    pub volume: u32,
    pub dog_mode: bool,
    /// the view model changes without an action: read it every frame
    pub live: bool,
    /// a match is showing: keys go to the core
    pub capture: bool,
    /// the input bit of each key code while `capture` is on
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub keys: BTreeMap<String, u32>,
    /// names the match whose scene the page should have open, or nothing
    pub scene: String,
    /// the address's `#room=CODE` while this browser is in a room
    pub hash: String,
    /// a status line, or nothing
    pub status: String,
    /// where the panels sit (`dock.rs`)
    pub dock: Dock,
    /// every seated character on the lobby screen, the winning team on the
    /// winners' screen
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lineup: Vec<LineupSeat>,
    /// the winners' screen: the lineup celebrates, and the tune plays on
    #[serde(skip_serializing_if = "is_false")]
    pub cheer: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_item_says_its_type_and_leaves_out_what_is_off() {
        let b = Item::Button {
            id: "a".into(),
            text: "A".into(),
            quiet: false,
            disabled: false,
        };
        assert_eq!(
            serde_json::to_string(&b).unwrap(),
            r#"{"type":"button","id":"a","text":"A"}"#
        );
    }
}
