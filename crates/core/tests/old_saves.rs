//! Saves kit: every save a deployed build wrote still loads.
//!
//! `tests/saves/` holds one save written by each build that was deployed,
//! named for its version (`v1.json`, `v2-shop.json`). Before each deploy the
//! project adds one from the build being deployed. This fails when a save
//! from any of them is refused, when no save from this version is there yet,
//! and when a field an old save holds does not come back with the same value
//! and is not listed in `save::DROPPED`.

use std::fs;
use std::path::Path;
use tool_core::save::{load, store, Loaded, DROPPED, VERSION};

fn fixtures() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/saves");
    let mut out: Vec<(String, String)> = fs::read_dir(&dir)
        .expect("tests/saves/ holds a save from each deployed build")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            (name, fs::read_to_string(&p).expect("read save"))
        })
        .collect();
    out.sort();
    out
}

#[test]
fn every_deployed_builds_save_still_loads() {
    let all = fixtures();
    assert!(!all.is_empty(), "tests/saves/ is empty");
    for (name, text) in &all {
        match load(text) {
            Loaded::Ready(_) => {}
            other => panic!("{name}: a save from a deployed build no longer loads: {other:?}"),
        }
    }
}

#[test]
fn a_save_from_this_version_is_kept_before_deploying() {
    let prefix = format!("v{VERSION}");
    assert!(
        fixtures().iter().any(|(n, _)| n == &format!("{prefix}.json") || n.starts_with(&format!("{prefix}-"))),
        "no save from version {VERSION} in tests/saves/: write one from this build (save.rs's store) before deploying"
    );
}

#[test]
fn no_field_of_an_old_save_is_lost_or_changed_without_saying_so() {
    for (name, text) in fixtures() {
        let old: serde_json::Value = serde_json::from_str(&text).expect("fixture is JSON");
        let from = old["version"].as_u64().unwrap_or(0) as u32;
        let Loaded::Ready(save) = load(&text) else {
            continue; // the test above reports it
        };
        let now: serde_json::Value = serde_json::from_str(&store(&save)).unwrap();
        for (key, value) in old.as_object().unwrap() {
            if key == "version" {
                continue;
            }
            let dropped = DROPPED.iter().any(|(k, v, _)| k == key && *v > from);
            assert!(
                now.get(key) == Some(value) || dropped,
                "{name}: the field {key:?} is gone or changed after loading ({value} became {}); carry it over in migrate, or list it in DROPPED with the reason",
                now.get(key).map_or("nothing".to_string(), |v| v.to_string())
            );
        }
    }
}
