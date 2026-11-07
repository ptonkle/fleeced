//! Bots against bots with no page: PLAN 19's table of who wins and how long
//! it takes. Each row is the lobby's defaults for that many Wolves.
//!
//!   cargo run --release -p tool_core --example bot_table [matches] [DvW ...]
//!
//! `bot_table 6 2v2` plays six matches of two against two; with no list it
//! plays the five line-ups of PLAN 19. To try a lobby of other numbers, set
//! `BOT_TABLE_SHEEP`, `BOT_TABLE_EATS` and `BOT_TABLE_FOLD` (a percent of the
//! sheep, with the rule's cap).

use std::sync::Arc;

use tool_core::game::Match;
use tool_core::lobby::{MatchStart, Role, StartSeat};
use tool_core::map::Map;
use tool_core::online::SEATS;
use tool_core::rules::outcome::Outcome;
use tool_core::rules::setup::Settings;
use tool_core::rules::Role as Team;

const LINE_UPS: [(usize, usize); 5] = [(1, 1), (2, 2), (4, 4), (1, 7), (7, 1)];

fn env_number(name: &str) -> Option<i32> {
    std::env::var(name).ok().and_then(|n| n.parse().ok())
}

fn play(dogs: usize, wolves: usize, seed: u64) -> (Option<Outcome>, i32, i32, i32, i32, i32) {
    let picks = (0..dogs + wolves).map(|k| if k < dogs { Role::Sheepdog } else { Role::Wolf });
    let seats = picks.enumerate().map(|(k, role)| StartSeat {
        seat: k as u32,
        role,
        person: String::new(),
        name: String::new(),
    });
    let mut settings = Settings::default();
    settings.refill(wolves as i32);
    if let Some(eats) = env_number("BOT_TABLE_EATS") {
        settings.set_eats_required(eats);
    }
    if let Some(sheep) = env_number("BOT_TABLE_SHEEP") {
        settings.set_sheep(sheep);
    }
    let start = MatchStart {
        seed,
        seats: seats.collect(),
        settings,
    };
    let (sheep, eats_required) = (start.settings.sheep, start.settings.eats_required);
    let mut game = Match::new(&start, Arc::new(Map::generate(seed)));
    if let Some(percent) = env_number("BOT_TABLE_FOLD") {
        game.goals.fold_target = ((sheep * percent + 99) / 100).min(sheep - eats_required + 1);
    }
    let trace = std::env::var_os("BOT_TABLE_TRACE").is_some();
    let (mut kills, mut alive) = (0, wolves as i32);
    while game.outcome().is_none() {
        game.step(&[None; SEATS]);
        let now = game.standing();
        kills += (now.wolves_alive < alive) as i32;
        alive = now.wolves_alive;
        if trace && now.elapsed % 1800 == 0 {
            let body = &game.world.bodies[0];
            let (mut fields, mut in_fold) = ([0; 4], 0);
            for s in game.world.flock.sheep.iter().filter(|s| !s.eaten) {
                fields[s.field as usize & 3] += 1;
                in_fold += s.folded as i32;
            }
            let (x, z) = (body.x >> 16, body.z >> 16);
            println!(
                "  {} s: fields {fields:?} folded {in_fold} eats {} dog ({x}, {z})",
                now.elapsed / 60,
                now.eats
            );
        }
    }
    let end = game.standing();
    let goal = game.goals.fold_target;
    (
        game.outcome(),
        end.elapsed / 60,
        end.eats,
        end.in_fold,
        goal,
        kills,
    )
}

fn main() {
    let mut args = std::env::args().skip(1);
    let matches: u64 = args.next().and_then(|n| n.parse().ok()).unwrap_or(6);
    let asked: Vec<(usize, usize)> = args
        .filter_map(|a| {
            let (d, w) = a.split_once('v')?;
            Some((d.parse().ok()?, w.parse().ok()?))
        })
        .collect();
    let line_ups = if asked.is_empty() {
        LINE_UPS.to_vec()
    } else {
        asked
    };
    println!("| Line-up | Matches | Sheepdogs win | By the clock | Wolves win | Mean length | Shortest to longest | Mean eats | Mean in the Fold | Wolves killed, per match |");
    println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    for (dogs, wolves) in line_ups {
        let (mut won, mut clock, mut seconds, mut eats, mut folded, mut lunges) = (0, 0, 0, 0, 0, 0);
        let (mut least, mut most) = (i32::MAX, 0);
        for seed in 1..=matches {
            let (outcome, length, ate, in_fold, goal, killed) = play(dogs, wolves, seed);
            lunges += killed as u64;
            let ended = outcome.expect("played out");
            let winner = ended.winner();
            clock += (ended == Outcome::TimeRanOut) as u64;
            if std::env::var_os("BOT_TABLE_EACH").is_some() {
                println!("{dogs}v{wolves} seed {seed}: {outcome:?} {length} s, eats {ate}, fold {in_fold} of {goal}");
            }
            won += (winner == Team::Sheepdog) as u64;
            seconds += length as u64;
            eats += ate as u64;
            folded += in_fold as u64;
            least = least.min(length);
            most = most.max(length);
        }
        let mean = |n: u64| n / matches;
        println!(
            "| {dogs} v {wolves} | {matches} | {won} | {clock} | {} | {} s | {least} to {most} s | {} | {} | {} |",
            matches - won,
            mean(seconds),
            mean(eats),
            mean(folded),
            mean(lunges),
        );
    }
}
