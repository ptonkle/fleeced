//! A playtest file a player can send back. Part of the saves kit.
//!
//! A friend who plays the build can press "Send a playtest file" and pass the
//! file on. It holds counts of named events and the tick each milestone was
//! first reached, nothing else: no names, no free text, nothing typed. The
//! event and milestone names are fixed here, so a call with any other name
//! is ignored and cannot carry something personal into the file.
//! `tools/playtest_readout.py` turns a pile of these into a table for the
//! design journal.

use serde::Serialize;
use std::collections::BTreeMap;

use crate::save::VERSION;

/// The events worth counting in a match.
pub const EVENTS: &[&str] = &[
    "match_started",
    "sheep_eaten",
    "wolf_killed",
    "sheep_folded",
    "powerup_dug",
];

/// Milestones whose first tick is worth knowing.
pub const MILESTONES: &[&str] = &["first_eat", "first_win"];

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Playtest {
    pub ticks: u64,
    pub sessions: u32,
    pub counts: BTreeMap<&'static str, u64>,
    pub firsts: BTreeMap<&'static str, u64>,
}

impl Playtest {
    pub fn tick(&mut self) {
        self.ticks += 1;
    }

    pub fn session(&mut self) {
        self.sessions += 1;
    }

    /// Counts a named event; any other name is ignored.
    pub fn count(&mut self, event: &str) {
        if let Some(name) = EVENTS.iter().copied().find(|e| *e == event) {
            *self.counts.entry(name).or_insert(0) += 1;
        }
    }

    /// Records the tick a milestone was first reached; later ones are ignored.
    pub fn reached(&mut self, milestone: &str) {
        if let Some(name) = MILESTONES.iter().copied().find(|m| *m == milestone) {
            self.firsts.entry(name).or_insert(self.ticks);
        }
    }

    /// The file's text.
    pub fn file(&self) -> String {
        #[derive(Serialize)]
        struct Sheet<'a> {
            kind: &'static str,
            save_version: u32,
            #[serde(flatten)]
            body: &'a Playtest,
        }
        serde_json::to_string(&Sheet {
            kind: "playtest",
            save_version: VERSION,
            body: self,
        })
        .expect("a Playtest always serializes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_named_events_and_milestones_reach_the_file() {
        let mut p = Playtest::default();
        p.session();
        p.count("sheep_eaten");
        p.count("sheep_eaten");
        p.count("Sam typed this");
        p.tick();
        p.tick();
        p.reached("first_win");
        p.tick();
        p.reached("first_win");
        p.reached("my email");
        assert_eq!(
            p.file(),
            format!(
                r#"{{"kind":"playtest","save_version":{VERSION},"ticks":3,"sessions":1,"counts":{{"sheep_eaten":2}},"firsts":{{"first_win":2}}}}"#
            )
        );
    }
}
