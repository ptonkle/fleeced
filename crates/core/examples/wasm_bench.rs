//! The bench's worlds behind a few plain exports, so `scripts/bench_wasm.mjs`
//! can time one `World::step` in WebAssembly under node, with no bindings in
//! the way.

mod scene;

use scene::{bytes, inputs, scene, PLAYERS};
use std::cell::RefCell;
use tool_core::body::Input;
use tool_core::map::Map;
use tool_core::world::World;

struct State {
    map: Map,
    world: World,
    saved: World,
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
    state.tick += 1;
    state.world.step(&state.map, &keys);
}

/// Build a world: `which` of `scene::SCENES` with this many sheep. The map
/// carries over between calls; a new one costs a noticeable moment.
#[no_mangle]
pub extern "C" fn load(which: u32, sheep: u32) {
    STATE.with(|state| {
        let mut slot = state.borrow_mut();
        let map = match slot.take() {
            Some(old) => old.map,
            None => Map::generate(1),
        };
        let world = scene(&map, which as usize, sheep as usize);
        let saved = world.clone();
        *slot = Some(State {
            map,
            world,
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
    with(|state| state.saved = state.world.clone());
}

/// Back to the saved world and forward again, as a rollback does.
#[no_mangle]
pub extern "C" fn replay(count: u32) {
    with(|state| {
        state.world = state.saved.clone();
        for _ in 0..count {
            advance(state);
        }
    });
}

#[no_mangle]
pub extern "C" fn world_bytes() -> u32 {
    with(|state| bytes(&state.world) as u32)
}
