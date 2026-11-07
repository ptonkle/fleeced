//! `wasm_bench.rs` for a whole `Match::step`: the rules and the map's checks
//! around the world, on the same scenes, with every button in use. Same
//! exports, so `scripts/bench_wasm.mjs` times it:
//!
//!   cargo build --release --target wasm32-unknown-unknown -p tool_core --example wasm_match
//!   node scripts/bench_wasm.mjs target/wasm32-unknown-unknown/release/examples/wasm_match.wasm
//!
//! With `WASM_MATCH_BOTS=1` on the build, bots hold seven of the eight seats.

mod scene;

use scene::{bytes, inputs, scene, PLAYERS};
use std::cell::RefCell;
use std::sync::Arc;
use tool_core::body::{Input, ACT, PRIMARY, SECONDARY};
use tool_core::game::Match;
use tool_core::lobby::{MatchStart, Role, StartSeat};
use tool_core::map::Map;
use tool_core::online::{Rollback, SEATS};
use tool_core::rules::setup::Settings;
use tool_core::rules::Role as Team;

/// How many seats a person holds. Built with `WASM_MATCH_BOTS` set, one does
/// and bots hold the other seven.
const PEOPLE: usize = if option_env!("WASM_MATCH_BOTS").is_some() {
    1
} else {
    SEATS
};

struct State {
    map: Arc<Map>,
    game: Match,
    saved: Match,
    tick: u32,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn with<T>(run: impl FnOnce(&mut State) -> T) -> T {
    STATE.with(|state| run(state.borrow_mut().as_mut().expect("load first")))
}

fn advance(state: &mut State) {
    let mut keys = [Input::default(); PLAYERS];
    inputs(state.tick, &mut keys);
    for (k, input) in keys.iter_mut().enumerate() {
        let t = state.tick + k as u32 * 11;
        let tap = |every: u32, button: u16| if t.is_multiple_of(every) { button } else { 0 };
        input.buttons |= tap(50, ACT) | tap(70, PRIMARY) | tap(90, SECONDARY);
    }
    state.tick += 1;
    let mut packed = [0; SEATS];
    for (k, (out, input)) in packed.iter_mut().zip(keys).enumerate() {
        *out = if k >= PEOPLE { Match::NOBODY } else { input.pack() };
    }
    Rollback::step(&mut state.game, packed);
}

/// A match on `which` of `scene::SCENES` with this many sheep, long enough
/// and with goals high enough to outlast the bench.
fn build(map: &Arc<Map>, which: usize, sheep: usize) -> Match {
    let world = scene(map, which, sheep);
    let seats = world.bodies.iter().enumerate().map(|(k, body)| StartSeat {
        seat: k as u32,
        role: match body.role {
            Team::Sheepdog => Role::Sheepdog,
            Team::Wolf => Role::Wolf,
        },
        person: format!("p{k}"),
        name: String::new(),
    });
    let start = MatchStart {
        seed: 1,
        seats: seats.collect(),
        settings: Settings {
            match_minutes: 30,
            sheep: sheep as i32,
            eats_required: sheep as i32,
            ..Settings::default()
        },
    };
    let mut game = Match::new(&start, map.clone());
    game.world = world;
    game.goals.fold_target = sheep as i32 + 1;
    game.goals.eats_required = sheep as i32 + 1;
    game
}

#[no_mangle]
pub extern "C" fn load(which: u32, sheep: u32) {
    STATE.with(|state| {
        let mut slot = state.borrow_mut();
        let map = match slot.take() {
            Some(old) => old.map,
            None => Arc::new(Map::generate(1)),
        };
        let game = build(&map, which as usize, sheep as usize);
        let saved = game.clone();
        *slot = Some(State {
            map,
            game,
            saved,
            tick: 0,
        });
    });
}

#[no_mangle]
pub extern "C" fn steps(count: u32) {
    with(|state| {
        for _ in 0..count {
            advance(state);
        }
    });
}

#[no_mangle]
pub extern "C" fn save() {
    with(|state| state.saved = state.game.clone());
}

/// Back to the saved match and forward again, as a rollback does.
#[no_mangle]
pub extern "C" fn replay(count: u32) {
    with(|state| {
        state.game = state.saved.clone();
        for _ in 0..count {
            advance(state);
        }
    });
}

#[no_mangle]
pub extern "C" fn world_bytes() -> u32 {
    with(|state| bytes(&state.game.world) as u32)
}
