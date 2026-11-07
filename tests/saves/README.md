# Saves from deployed builds

One save written by each build that was deployed, named for its save version (`v1.json`, `v2-shop.json`). `crates/core/tests/old_saves.rs` loads every one. Before each deploy, write a save from the build being deployed (the page's "Save to file", or `save::store`) into this folder, and commit it with the deploy. Never edit one by hand: it stands for a player's real file.
