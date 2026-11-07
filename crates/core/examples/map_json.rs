//! Prints a map as JSON for `tools/map/render_seed.py`, which holds no map
//! rule of its own: everything it draws comes from here.
//!
//!     cargo run --release --example map_json -- SEED [SHEEP]

use tool_core::map::Map;
use tool_core::map::{proof, table};

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: u64 = args.next().and_then(|a| a.parse().ok()).unwrap_or(1);
    let sheep: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(100);
    let map = Map::generate(seed);
    let out = serde_json::json!({
        "map": map,
        "length": table::LENGTH,
        "width": table::WIDTH,
        "sheep": map.sheep_starts(sheep),
        "route_width": proof::route_width(&map),
    });
    println!("{out}");
}
