//! Who has won. `check` runs once a tick, after that tick's events, and the
//! match stops at the first answer.
//!
//! A tie goes to the Wolves. Eats are checked first, and that is the whole
//! rule: an eat on the last tick is inside the time limit, and when an eat
//! and a sheep reaching the Fold land on one tick, the eat stands.

use serde::{Deserialize, Serialize};

use super::Role;

/// What ends the match, fixed at match start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Goals {
    pub eats_required: i32,
    pub fold_target: i32,
    pub match_ticks: i32,
    pub sudden_death: bool,
}

/// The match as it stands after a tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Standing {
    pub eats: i32,
    pub in_fold: i32,
    pub carcasses_found: i32,
    /// Wolves on the map now, not counting any waiting to return.
    pub wolves_alive: i32,
    /// Ticks played, counting the one just run.
    pub elapsed: i32,
}

/// How a match ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Outcome {
    WolvesAte,
    FoldFilled,
    LastWolfDead,
    TimeRanOut,
}

impl Outcome {
    pub fn winner(self) -> Role {
        match self {
            Outcome::WolvesAte => Role::Wolf,
            Outcome::FoldFilled | Outcome::LastWolfDead | Outcome::TimeRanOut => Role::Sheepdog,
        }
    }
}

pub fn check(goals: &Goals, now: &Standing) -> Option<Outcome> {
    if now.eats >= goals.eats_required {
        return Some(Outcome::WolvesAte);
    }
    if now.in_fold >= goals.fold_target {
        return Some(Outcome::FoldFilled);
    }
    if goals.sudden_death && now.wolves_alive == 0 {
        return Some(Outcome::LastWolfDead);
    }
    if now.elapsed >= goals.match_ticks {
        return Some(Outcome::TimeRanOut);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOALS: Goals = Goals {
        eats_required: 10,
        fold_target: 75,
        match_ticks: 28_800,
        sudden_death: false,
    };
    const SUDDEN: Goals = Goals {
        sudden_death: true,
        ..GOALS
    };
    const PLAYING: Standing = Standing {
        eats: 9,
        in_fold: 74,
        carcasses_found: 0,
        wolves_alive: 1,
        elapsed: 28_799,
    };

    #[test]
    fn nothing_ends_a_match_one_short_of_every_goal() {
        assert_eq!(check(&GOALS, &PLAYING), None);
        assert_eq!(check(&SUDDEN, &PLAYING), None);
    }

    #[test]
    fn wolves_win_when_eats_reach_the_requirement() {
        let now = Standing { eats: 10, ..PLAYING };
        assert_eq!(check(&GOALS, &now), Some(Outcome::WolvesAte));
    }

    #[test]
    fn sheepdogs_win_when_the_fold_reaches_its_target() {
        let now = Standing {
            in_fold: 75,
            ..PLAYING
        };
        assert_eq!(check(&GOALS, &now), Some(Outcome::FoldFilled));
    }

    #[test]
    fn sheepdogs_win_when_time_runs_out() {
        let now = Standing {
            elapsed: 28_800,
            ..PLAYING
        };
        assert_eq!(check(&GOALS, &now), Some(Outcome::TimeRanOut));
    }

    #[test]
    fn under_sudden_death_the_last_wolf_dying_ends_the_match() {
        let now = Standing {
            wolves_alive: 0,
            ..PLAYING
        };
        assert_eq!(check(&SUDDEN, &now), Some(Outcome::LastWolfDead));
    }

    #[test]
    fn with_sudden_death_off_no_wolves_on_the_map_ends_nothing() {
        let now = Standing {
            wolves_alive: 0,
            ..PLAYING
        };
        assert_eq!(check(&GOALS, &now), None);
    }

    #[test]
    fn an_eat_on_the_last_tick_beats_the_clock() {
        let now = Standing {
            eats: 10,
            elapsed: 28_800,
            ..PLAYING
        };
        assert_eq!(check(&GOALS, &now), Some(Outcome::WolvesAte));
    }

    #[test]
    fn an_eat_on_the_tick_the_fold_fills_goes_to_the_wolves() {
        let now = Standing {
            eats: 10,
            in_fold: 75,
            ..PLAYING
        };
        assert_eq!(check(&GOALS, &now), Some(Outcome::WolvesAte));
    }

    #[test]
    fn each_ending_names_its_winner() {
        assert_eq!(Outcome::WolvesAte.winner(), Role::Wolf);
        assert_eq!(Outcome::FoldFilled.winner(), Role::Sheepdog);
        assert_eq!(Outcome::LastWolfDead.winner(), Role::Sheepdog);
        assert_eq!(Outcome::TimeRanOut.winner(), Role::Sheepdog);
    }
}
