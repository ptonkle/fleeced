//! Match setup: the settings, the seats, Random roles, the lobby's defaults
//! and the Fold target. What comes out of `start_match` is fixed for the match.

use serde::{Deserialize, Serialize};

use super::outcome::Goals;
use super::tune::{self, secs};
use super::Role;
use crate::map::MapSize;
use crate::rng::Rng;

/// What the lobby refills as teams change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LobbyDefaults {
    pub sheep: i32,
    pub eats_required: i32,
}

/// Number of sheep and Eats required for this many Wolves. The lobby calls
/// this. Eats required is 8 per Wolf; Number of sheep is 100 plus 50 per Wolf
/// after the first. A lone Wolf has its own numbers, 80 sheep and 9 eats. A lobby with nobody on the Wolves yet reads as one Wolf,
/// and one with eight (which can't start) as seven.
pub fn lobby_defaults(wolves: i32) -> LobbyDefaults {
    let wolves = wolves.clamp(1, tune::MAX_SEATS as i32 - 1);
    if wolves == 1 {
        return LobbyDefaults {
            sheep: tune::SHEEP_LONE_WOLF,
            eats_required: tune::EATS_LONE_WOLF,
        };
    }
    LobbyDefaults {
        sheep: tune::SHEEP_FIRST_WOLF + tune::SHEEP_PER_EXTRA_WOLF * (wolves - 1),
        eats_required: tune::EATS_PER_WOLF * wolves,
    }
}

/// How many sheep must reach the Fold: a number per Sheepdog, whatever the
/// Wolves or the flock, capped at sheep minus eats required plus 1.
///
/// The cap keeps both wins reachable and never both at once. One eat short of
/// Eats required, sheep - eats + 1 sheep are still alive, so the Sheepdogs can
/// still make the target; one sheep short of it, at least Eats required sheep
/// are still outside the Fold, so the Wolves can still make theirs.
pub fn fold_target(sheep: i32, eats_required: i32, sheepdogs: i32) -> i32 {
    (tune::FOLD_PER_SHEEPDOG * sheepdogs.max(1)).min(sheep - eats_required + 1)
}

/// The match settings the host sets in the lobby.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub match_minutes: i32,
    pub sheep: i32,
    pub eats_required: i32,
    pub powerups: bool,
    pub sudden_death: bool,
    /// Goes with the seed, so every browser builds the same map.
    #[serde(default)]
    pub map_size: MapSize,
    /// Whether the host typed this value. Once they have, `refill` leaves it.
    pub sheep_by_hand: bool,
    pub eats_by_hand: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        let filled = lobby_defaults(1);
        Settings {
            match_minutes: tune::MATCH_MINUTES,
            sheep: filled.sheep,
            eats_required: filled.eats_required,
            powerups: tune::POWERUPS_ON,
            sudden_death: tune::SUDDEN_DEATH,
            map_size: MapSize::default(),
            sheep_by_hand: false,
            eats_by_hand: false,
        }
    }
}

impl Settings {
    /// The host typed a Number of sheep.
    pub fn set_sheep(&mut self, sheep: i32) {
        self.sheep = sheep;
        self.sheep_by_hand = true;
    }

    /// The host typed an Eats required.
    pub fn set_eats_required(&mut self, eats_required: i32) {
        self.eats_required = eats_required;
        self.eats_by_hand = true;
    }

    /// Refills whatever the host hasn't typed, for this many Wolves. A
    /// refilled value gives way to a typed one it would clash with, so a match
    /// can't be refused on a number nobody chose once Random roles settle.
    pub fn refill(&mut self, wolves: i32) {
        let filled = lobby_defaults(wolves);
        if !self.sheep_by_hand {
            let floor = if self.eats_by_hand { self.eats_required } else { 0 };
            self.sheep = filled.sheep.max(floor).min(tune::SHEEP_MAX);
        }
        if !self.eats_by_hand {
            self.eats_required = filled.eats_required.min(self.sheep);
        }
    }

    pub fn validate(&self) -> Result<(), SetupError> {
        let (min, max) = (tune::MATCH_MINUTES_MIN, tune::MATCH_MINUTES_MAX);
        if !(min..=max).contains(&self.match_minutes) {
            return Err(SetupError::MatchLength { min, max });
        }
        let (min, max) = (tune::SHEEP_MIN, tune::SHEEP_MAX);
        if !(min..=max).contains(&self.sheep) {
            return Err(SetupError::Sheep { min, max });
        }
        let (min, max) = (tune::EATS_MIN, self.sheep);
        if !(min..=max).contains(&self.eats_required) {
            return Err(SetupError::EatsRequired { min, max });
        }
        Ok(())
    }
}

/// What a seat asked for in the lobby.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoleChoice {
    Sheepdog,
    Wolf,
    Random,
}

/// A seat in the lobby. A bot's seat counts like any other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbySeat {
    pub choice: RoleChoice,
    pub bot: bool,
}

/// A seat in the match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seat {
    pub role: Role,
    pub bot: bool,
}

impl Seat {
    /// The player left: a bot plays this seat from here on, on the same team.
    pub fn leave(&mut self) {
        self.bot = true;
    }
}

/// Why a match can't start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupError {
    MatchLength { min: i32, max: i32 },
    Sheep { min: i32, max: i32 },
    EatsRequired { min: i32, max: i32 },
    TooFewPlayers { min: usize },
    TooManyPlayers { max: usize },
    NoSheepdog,
    NoWolf,
}

impl SetupError {
    /// The message the lobby shows.
    pub fn message(&self) -> String {
        match self {
            SetupError::MatchLength { min, max } => format!("Set Match length from {min} to {max} minutes."),
            SetupError::Sheep { min, max } => format!("Set Number of sheep from {min} to {max}."),
            SetupError::EatsRequired { min, max } => {
                format!("Set Eats required from {min} to {max}, the Number of sheep.")
            }
            SetupError::TooFewPlayers { min } => format!("Too few players. Seat at least {min}."),
            SetupError::TooManyPlayers { max } => format!("Too many players. Seat at most {max}."),
            SetupError::NoSheepdog => "No Sheepdog. Seat one, or set a seat to Random.".to_string(),
            SetupError::NoWolf => "No Wolf. Seat one, or set a seat to Random.".to_string(),
        }
    }
}

/// Each seat's role, in seat order. Random seats flip a coin from the seed; if
/// that leaves a team empty, one Random seat, drawn from the seed, crosses over.
pub fn resolve_roles(seats: &[RoleChoice], seed: u64) -> Result<Vec<Role>, SetupError> {
    if seats.len() < tune::MIN_SEATS {
        return Err(SetupError::TooFewPlayers { min: tune::MIN_SEATS });
    }
    if seats.len() > tune::MAX_SEATS {
        return Err(SetupError::TooManyPlayers { max: tune::MAX_SEATS });
    }
    let random_seats: Vec<usize> = (0..seats.len())
        .filter(|&i| seats[i] == RoleChoice::Random)
        .collect();
    if random_seats.is_empty() && !seats.contains(&RoleChoice::Sheepdog) {
        return Err(SetupError::NoSheepdog);
    }
    if random_seats.is_empty() && !seats.contains(&RoleChoice::Wolf) {
        return Err(SetupError::NoWolf);
    }
    let mut rng = Rng::new(seed);
    let mut roles: Vec<Role> = seats
        .iter()
        .map(|seat| match seat {
            RoleChoice::Sheepdog => Role::Sheepdog,
            RoleChoice::Wolf => Role::Wolf,
            RoleChoice::Random if rng.below(2) == 0 => Role::Sheepdog,
            RoleChoice::Random => Role::Wolf,
        })
        .collect();
    for team in [Role::Sheepdog, Role::Wolf] {
        if !roles.contains(&team) {
            let draw = rng.below(random_seats.len() as u32) as usize;
            roles[random_seats[draw]] = team;
        }
    }
    Ok(roles)
}

/// Everything fixed at match start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MatchSetup {
    /// The seats, in lobby order.
    pub seats: Vec<Seat>,
    pub settings: Settings,
    pub goals: Goals,
}

/// What ends a match on these settled settings.
pub fn goals(settings: &Settings, sheepdogs: i32) -> Goals {
    Goals {
        eats_required: settings.eats_required,
        fold_target: fold_target(settings.sheep, settings.eats_required, sheepdogs),
        match_ticks: secs(settings.match_minutes * 60),
        sudden_death: settings.sudden_death,
    }
}

/// Starts a match: resolves Random seats from the seed, settles the lobby's
/// defaults on the Wolves that came out, and fixes the goals.
pub fn start_match(settings: &Settings, seats: &[LobbySeat], seed: u64) -> Result<MatchSetup, SetupError> {
    let choices: Vec<RoleChoice> = seats.iter().map(|seat| seat.choice).collect();
    let roles = resolve_roles(&choices, seed)?;
    let wolves = roles.iter().filter(|r| **r == Role::Wolf).count() as i32;
    let mut settled = *settings;
    settled.refill(wolves);
    settled.validate()?;
    let sheepdogs = roles.iter().filter(|r| **r == Role::Sheepdog).count() as i32;
    let goals = goals(&settled, sheepdogs);
    let seats = seats.iter().zip(&roles).map(|(seat, role)| Seat {
        role: *role,
        bot: seat.bot,
    });
    Ok(MatchSetup {
        seats: seats.collect(),
        settings: settled,
        goals,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::Role::{Sheepdog, Wolf};

    fn lobby(choices: &[RoleChoice]) -> Vec<LobbySeat> {
        let seat = |choice: &RoleChoice| LobbySeat {
            choice: *choice,
            bot: false,
        };
        choices.iter().map(seat).collect()
    }

    #[test]
    fn lobby_defaults_follow_the_wolf_count() {
        for (wolves, sheep, eats_required) in [(1, 80, 9), (2, 150, 16), (4, 250, 32), (7, 400, 56)] {
            let expected = LobbyDefaults { sheep, eats_required };
            assert_eq!(lobby_defaults(wolves), expected, "{wolves} wolves");
        }
    }

    #[test]
    fn lobby_defaults_read_no_wolves_as_one_and_eight_as_seven() {
        let one = LobbyDefaults {
            sheep: 80,
            eats_required: 9,
        };
        let seven = LobbyDefaults {
            sheep: 400,
            eats_required: 56,
        };
        assert_eq!(lobby_defaults(0), one);
        assert_eq!(lobby_defaults(8), seven);
    }

    #[test]
    fn the_fold_target_counts_only_the_sheepdogs() {
        assert_eq!(fold_target(80, 9, 1), 60);
        assert_eq!(fold_target(150, 16, 2), 120);
        // more Wolves, and so more sheep and eats, with the same two Sheepdogs
        assert_eq!(fold_target(250, 32, 2), 120);
        assert_eq!(fold_target(400, 56, 2), 120);
    }

    #[test]
    fn the_fold_target_is_capped_at_sheep_minus_eats_plus_one() {
        assert_eq!(fold_target(100, 80, 1), 21);
        assert_eq!(fold_target(100, 42, 1), 59);
        assert_eq!(fold_target(100, 41, 1), 60);
        assert_eq!(fold_target(100, 40, 1), 60);
        // seven Sheepdogs would want 420
        assert_eq!(fold_target(80, 9, 7), 72);
    }

    #[test]
    fn the_default_settings_are_the_persons_and_valid() {
        let s = Settings::default();
        assert_eq!((s.match_minutes, s.sheep, s.eats_required), (8, 80, 9));
        assert!(s.powerups);
        assert!(!s.sudden_death);
        assert_eq!(s.validate(), Ok(()));
    }

    #[test]
    fn match_length_runs_from_one_to_thirty_minutes() {
        let with = |match_minutes| {
            Settings {
                match_minutes,
                ..Settings::default()
            }
            .validate()
        };
        let refused = Err(SetupError::MatchLength { min: 1, max: 30 });
        assert_eq!(with(1), Ok(()));
        assert_eq!(with(30), Ok(()));
        assert_eq!(with(0), refused);
        assert_eq!(with(31), refused);
    }

    #[test]
    fn number_of_sheep_runs_from_ten_to_four_hundred() {
        let with = |sheep| {
            Settings {
                sheep,
                eats_required: 10,
                ..Settings::default()
            }
            .validate()
        };
        let refused = Err(SetupError::Sheep { min: 10, max: 400 });
        assert_eq!(with(10), Ok(()));
        assert_eq!(with(400), Ok(()));
        assert_eq!(with(9), refused);
        assert_eq!(with(401), refused);
    }

    #[test]
    fn eats_required_runs_from_one_to_the_number_of_sheep() {
        let with = |eats_required| {
            Settings {
                sheep: 100,
                eats_required,
                ..Settings::default()
            }
            .validate()
        };
        let refused = Err(SetupError::EatsRequired { min: 1, max: 100 });
        assert_eq!(with(1), Ok(()));
        assert_eq!(with(100), Ok(()));
        assert_eq!(with(0), refused);
        assert_eq!(with(101), refused);
    }

    #[test]
    fn the_lobby_refills_sheep_and_eats_when_teams_change() {
        let mut s = Settings::default();
        s.refill(4);
        assert_eq!((s.sheep, s.eats_required), (250, 32));
    }

    #[test]
    fn a_hand_edit_stops_the_refill_for_that_value_alone() {
        let mut s = Settings::default();
        s.set_sheep(120);
        s.refill(2);
        assert_eq!((s.sheep, s.eats_required), (120, 16));
        s.set_eats_required(15);
        s.refill(4);
        assert_eq!((s.sheep, s.eats_required), (120, 15));
    }

    #[test]
    fn a_refilled_value_gives_way_to_a_hand_edit_it_would_clash_with() {
        // one Wolf's 80 sheep would be fewer than the 120 eats typed
        let mut s = Settings::default();
        s.set_eats_required(120);
        s.refill(1);
        assert_eq!((s.sheep, s.eats_required), (120, 120));
        // four Wolves' 40 eats would be more than the 20 sheep typed
        let mut s = Settings::default();
        s.set_sheep(20);
        s.refill(4);
        assert_eq!((s.sheep, s.eats_required), (20, 20));
    }

    #[test]
    fn a_refill_never_lifts_sheep_past_the_limit() {
        let mut s = Settings::default();
        s.set_eats_required(500);
        s.refill(1);
        assert_eq!(s.sheep, 400);
        assert_eq!(s.validate(), Err(SetupError::EatsRequired { min: 1, max: 400 }));
    }

    #[test]
    fn a_match_needs_two_to_eight_seats() {
        let seats = [RoleChoice::Random; 9];
        assert_eq!(
            resolve_roles(&seats[..1], 0),
            Err(SetupError::TooFewPlayers { min: 2 })
        );
        assert_eq!(
            resolve_roles(&seats, 0),
            Err(SetupError::TooManyPlayers { max: 8 })
        );
        assert!(resolve_roles(&seats[..2], 0).is_ok());
        assert!(resolve_roles(&seats[..8], 0).is_ok());
    }

    #[test]
    fn a_team_with_no_seat_and_no_random_seat_is_refused() {
        let wolves = [RoleChoice::Wolf; 2];
        let sheepdogs = [RoleChoice::Sheepdog; 2];
        assert_eq!(resolve_roles(&wolves, 0), Err(SetupError::NoSheepdog));
        assert_eq!(resolve_roles(&sheepdogs, 0), Err(SetupError::NoWolf));
    }

    #[test]
    fn random_seats_resolve_from_the_seed() {
        // Worked out with a separate SplitMix64: the top bits of the first
        // four values are 1, 0, 0, 1 from seed 0 and 0, 1, 1, 0 from seed 3
        let seats = [RoleChoice::Random; 4];
        assert_eq!(resolve_roles(&seats, 0), Ok(vec![Wolf, Sheepdog, Sheepdog, Wolf]));
        assert_eq!(resolve_roles(&seats, 3), Ok(vec![Sheepdog, Wolf, Wolf, Sheepdog]));
    }

    #[test]
    fn chosen_seats_keep_their_choice_around_a_random_one() {
        // seed 0's first flip is 1, a Wolf
        let seats = [RoleChoice::Sheepdog, RoleChoice::Random, RoleChoice::Wolf];
        assert_eq!(resolve_roles(&seats, 0), Ok(vec![Sheepdog, Wolf, Wolf]));
    }

    #[test]
    fn a_random_seat_crosses_over_when_chance_empties_a_team() {
        // seed 0's flip gives a second Wolf, so the one Random seat crosses
        let seats = [RoleChoice::Wolf, RoleChoice::Random];
        assert_eq!(resolve_roles(&seats, 0), Ok(vec![Wolf, Sheepdog]));
        // seed 7's flip gives a second Sheepdog
        let seats = [RoleChoice::Sheepdog, RoleChoice::Random];
        assert_eq!(resolve_roles(&seats, 7), Ok(vec![Sheepdog, Wolf]));
    }

    #[test]
    fn the_seat_that_crosses_over_is_drawn_from_the_seed() {
        // flips 1, 1 are two Wolves; the third value picks seat 1 from seed 2
        // and seat 0 from seed 9
        let seats = [RoleChoice::Random; 2];
        assert_eq!(resolve_roles(&seats, 2), Ok(vec![Wolf, Sheepdog]));
        assert_eq!(resolve_roles(&seats, 9), Ok(vec![Sheepdog, Wolf]));
    }

    #[test]
    fn the_defaults_settle_on_the_wolves_that_come_out_of_random() {
        // seed 0 deals two Wolves from four Random seats
        let seats = lobby(&[RoleChoice::Random; 4]);
        let setup = start_match(&Settings::default(), &seats, 0).unwrap();
        let roles: Vec<Role> = setup.seats.iter().map(|seat| seat.role).collect();
        assert_eq!(roles, [Wolf, Sheepdog, Sheepdog, Wolf]);
        assert_eq!((setup.settings.sheep, setup.settings.eats_required), (150, 16));
        // 8 min at 60 s and 60 ticks is 28800 ticks; two Sheepdogs at 60 each
        let goals = Goals {
            eats_required: 16,
            fold_target: 120,
            match_ticks: 28_800,
            sudden_death: false,
        };
        assert_eq!(setup.goals, goals);
    }

    #[test]
    fn sudden_death_carries_into_the_goals() {
        let settings = Settings {
            sudden_death: true,
            ..Settings::default()
        };
        let seats = lobby(&[RoleChoice::Sheepdog, RoleChoice::Wolf]);
        assert!(start_match(&settings, &seats, 0).unwrap().goals.sudden_death);
    }

    #[test]
    fn a_match_will_not_start_on_settings_out_of_range() {
        let settings = Settings {
            match_minutes: 0,
            ..Settings::default()
        };
        let seats = lobby(&[RoleChoice::Sheepdog, RoleChoice::Wolf]);
        let refused = Err(SetupError::MatchLength { min: 1, max: 30 });
        assert_eq!(start_match(&settings, &seats, 0), refused);
    }

    #[test]
    fn a_bot_seat_stays_a_bot_seat_into_the_match() {
        let mut seats = lobby(&[RoleChoice::Sheepdog, RoleChoice::Wolf]);
        seats[1].bot = true;
        let setup = start_match(&Settings::default(), &seats, 0).unwrap();
        assert!(!setup.seats[0].bot);
        assert!(setup.seats[1].bot);
    }

    #[test]
    fn a_player_who_leaves_becomes_a_bot_on_the_same_team() {
        let mut seat = Seat {
            role: Wolf,
            bot: false,
        };
        seat.leave();
        assert_eq!(
            seat,
            Seat {
                role: Wolf,
                bot: true
            }
        );
    }

    #[test]
    fn each_refusal_says_what_to_do() {
        let cases = [
            (
                SetupError::MatchLength { min: 1, max: 30 },
                "Set Match length from 1 to 30 minutes.",
            ),
            (
                SetupError::Sheep { min: 10, max: 400 },
                "Set Number of sheep from 10 to 400.",
            ),
            (
                SetupError::EatsRequired { min: 1, max: 100 },
                "Set Eats required from 1 to 100, the Number of sheep.",
            ),
            (
                SetupError::TooFewPlayers { min: 2 },
                "Too few players. Seat at least 2.",
            ),
            (
                SetupError::TooManyPlayers { max: 8 },
                "Too many players. Seat at most 8.",
            ),
            (
                SetupError::NoSheepdog,
                "No Sheepdog. Seat one, or set a seat to Random.",
            ),
            (SetupError::NoWolf, "No Wolf. Seat one, or set a seat to Random."),
        ];
        for (error, message) in cases {
            assert_eq!(error.message(), message);
        }
    }
}
