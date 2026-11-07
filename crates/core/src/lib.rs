//! tool_core: every application rule of the tool.
//!
//! Builder requirement: this crate performs no input or output. It takes
//! values and returns values, so `cargo test` reaches every rule without a
//! browser. `tests/boundary.rs` enforces that, along with the dependency
//! allowlist and the reference ban on floats, hash maps and clocks.
//!
//! Keep `api.rs` as the one door the shim calls through.

pub mod api;
pub mod app;
pub mod body;
pub mod dock;
pub mod flock;
pub mod fx;
pub mod gallery;
pub mod game;
pub mod glossary;
pub mod ground;
pub mod hud;
pub mod icon;
pub mod lobby;
pub mod map;
pub mod mesh;
pub mod models;
pub mod online;
pub mod patch_notes;
pub mod playtest;
pub mod render;
pub mod rng;
pub mod rules;
pub mod save;
pub mod scape;
pub mod scene;
pub mod sound;
pub mod strings;
pub mod ticks;
pub mod tune;
pub mod tunes;
pub mod view;
pub mod world;
