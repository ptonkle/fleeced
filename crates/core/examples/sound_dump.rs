//! Prints every compiled sound as JSON, for tools/sound/render.py: the
//! offline render reads the core's output and recomputes no rule.

fn main() {
    let bank = tool_core::sound::bank().expect("data/sound.json reads");
    println!("{}", bank.dump().expect("every sound compiles"));
}
