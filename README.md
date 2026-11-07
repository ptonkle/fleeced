# Fleeced

Sheepdogs herd the flock across a Scottish hillside and into the Fold. Wolves hide among the sheep and eat them before they get there. Share a room code to play with friends, and give any empty seat to a bot.

This game is built from the Builder template. Its rules are in `BUILDER-SPEC.md`, a coding agent's instructions are in `CLAUDE.md` and `AGENTS.md`, the meaning of each term is in `GLOSSARY.md`, and the rules for every string are in `TONE.md`.

## Build, test and serve

```bash
./scripts/setup.sh              # once: Python venv with Playwright and three browsers
cargo test --workspace          # the core, boundary, copy and storage tests, in seconds
./scripts/build.sh              # release WebAssembly build into dist/
python3 scripts/serve.py        # http://127.0.0.1:8000/
./scripts/check.sh              # everything, before publishing
python3 scripts/break_tests.py  # each row of tests/breaks.json must make its test fail
./scripts/count-tests.sh        # the test count to quote in a document
./scripts/publish.sh            # stage 8: push dist/ to gh-pages, from a clean tree
python3 tools/journal/new_journal.py  # once: start the design journal beside this folder
```

Needs Rust (pinned in `rust-toolchain.toml`), `wasm-bindgen-cli` 0.2.127, and Python 3.

## Layout

| Path | Holds |
| --- | --- |
| `crates/core` | every application rule; no input or output; `api.rs` is the door: `start`, and `act(name, value)` |
| `crates/shim` | the `wasm-bindgen` functions the page calls; each is one call into the core |
| `web/` | the static page: a generic renderer for the core's view model |
| `web/net/` | the online transport: WebRTC, the relay list and the vendored Trystero bundle |
| `web/save/` | the one file that names browser storage |
| `data/` | reader-visible text, presentation numbers, shaders |
| `GLOSSARY.md`, `TONE.md` | what each term means; how every string is written |
| `tools/journal/` | the design journal's page, its entry kinds, and the script that starts it in `../<this folder>-journal/` |
| `tests/browser/` | the browser check, this game's interactions, and the two-browser online check |
| `tests/breaks.json` | one deliberate break per rule, for `scripts/break_tests.py` |
| `scripts/` | build, check, serve, publish, import audit, mutation audit, test count |
