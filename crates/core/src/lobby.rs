//! The lobby: who sits where, each seat's role, and the host's match
//! settings. Pure rules with no transport: `online::room` carries the lobby
//! between browsers, and a Solo match uses the same lobby with no one else in
//! it. The host's copy is the true one; a joiner draws the last roster it got.
//!
//! The lobby holds who sits where; `rules::setup` holds what a match means.
//! Number of sheep and Eats required follow the wolf count through
//! `Settings::refill`, each until the host types it, and `Lobby::start` asks
//! `rules::setup::start_match` to settle the Random seats. What the lobby
//! adds, each with a test below:
//!
//! - each seat is empty, a person or a bot, and a bot gives way to a person;
//! - a match needs at least one Sheepdog and one Wolf, and a Random seat can
//!   be either, so the check counts what Random seats can still become;
//! - every person but the host has pressed Ready;
//! - with Defaults on, Map size follows the number of players and the host
//!   can't type the three values Defaults fills.

use serde::{Deserialize, Serialize};

use crate::map::MapSize;
use crate::rules::setup::{map_size_for, start_match, LobbySeat, RoleChoice, Settings};
use crate::rules::{tune, Role as Team};

/// The most seats in a match, people and bots together.
pub const MAX_SEATS: usize = tune::MAX_SEATS;
/// The longest player name, in characters.
pub const MAX_NAME_CHARS: usize = 16;

/// The limits the host may set, from the rules' tune table.
pub const MINUTES: (i32, i32) = (tune::MATCH_MINUTES_MIN, tune::MATCH_MINUTES_MAX);
pub const SHEEP: (i32, i32) = (tune::SHEEP_MIN, tune::SHEEP_MAX);
pub const EATS: (i32, i32) = (tune::EATS_MIN, tune::SHEEP_MAX);

/// A status key: a string in `data/` says what stands in the way.
pub const NEED_BOTH: &str = "lobby.need_both";
pub const NEED_READY: &str = "lobby.need_ready";

/// What a seat plays as. `Random` settles at match start.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Sheepdog,
    Wolf,
    #[default]
    Random,
}

impl Role {
    /// The role's key: the action value, the stylesheet's and the lineup's
    /// name for it, and `role.<key>` in the strings.
    pub fn key(self) -> &'static str {
        match self {
            Role::Sheepdog => "sheepdog",
            Role::Wolf => "wolf",
            Role::Random => "random",
        }
    }

    pub fn parse(text: &str) -> Option<Role> {
        match text {
            "sheepdog" => Some(Role::Sheepdog),
            "wolf" => Some(Role::Wolf),
            "random" => Some(Role::Random),
            _ => None,
        }
    }

    /// The rules' word for a seat's choice.
    fn choice(self) -> RoleChoice {
        match self {
            Role::Sheepdog => RoleChoice::Sheepdog,
            Role::Wolf => RoleChoice::Wolf,
            Role::Random => RoleChoice::Random,
        }
    }
}

impl From<Team> for Role {
    fn from(team: Team) -> Role {
        match team {
            Team::Sheepdog => Role::Sheepdog,
            Team::Wolf => Role::Wolf,
        }
    }
}

/// Who sits in a seat.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "k", rename_all = "snake_case")]
pub enum Who {
    #[default]
    Empty,
    Person {
        /// the browser's client id, stable for its tab
        id: String,
        name: String,
        /// Ready. The host's own mark is unused: its Start is its Ready.
        ready: bool,
    },
    Bot,
}

/// One seat.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seat {
    pub who: Who,
    pub role: Role,
}

/// One seat as a match starts: its role settled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartSeat {
    pub seat: u32,
    pub role: Role,
    /// the person's client id, empty for a bot
    pub person: String,
    pub name: String,
}

/// Everything every browser needs to build the same match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchStart {
    pub seed: u64,
    pub seats: Vec<StartSeat>,
    pub settings: Settings,
}

/// The room before a match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lobby {
    pub seats: Vec<Seat>,
    pub settings: Settings,
    /// the host's client id
    pub host: String,
}

/// A name as it is kept: no control characters, no spaces at the ends, and
/// at most [`MAX_NAME_CHARS`] characters.
pub fn clean_name(text: &str) -> String {
    let kept: String = text.chars().filter(|c| !c.is_control()).collect();
    kept.trim()
        .chars()
        .take(MAX_NAME_CHARS)
        .collect::<String>()
        .trim()
        .to_string()
}

impl Lobby {
    /// A lobby with the host in the first seat.
    pub fn new(host_id: &str, host_name: &str) -> Lobby {
        let mut lobby = Lobby {
            seats: vec![Seat::default(); MAX_SEATS],
            settings: Settings::default(),
            host: host_id.to_string(),
        };
        lobby.seats[0].who = Who::Person {
            id: host_id.to_string(),
            name: clean_name(host_name),
            ready: false,
        };
        lobby.refill();
        lobby
    }

    /// A roster a host could have sent: the right number of seats.
    pub fn is_whole(&self) -> bool {
        self.seats.len() == MAX_SEATS
    }

    /// The seat a client id sits in.
    pub fn seat_of(&self, id: &str) -> Option<usize> {
        self.seats
            .iter()
            .position(|s| matches!(&s.who, Who::Person { id: i, .. } if i == id))
    }

    fn count(&self, role: Role) -> u32 {
        self.seats
            .iter()
            .filter(|s| s.who != Who::Empty && s.role == role)
            .count() as u32
    }

    /// The seats someone sits in, people and bots.
    pub fn players(&self) -> i32 {
        self.seats.iter().filter(|s| s.who != Who::Empty).count() as i32
    }

    /// Follows the wolf count. Random seats do not count until they settle,
    /// and a lobby with no sure Wolf reads as one. Two typed fields can still
    /// cross, so Eats required never passes the number of sheep here.
    fn refill(&mut self) {
        self.settings.refill(self.count(Role::Wolf) as i32);
        self.settings.eats_required = self.settings.eats_required.min(self.settings.sheep);
        if self.settings.defaults {
            self.settings.map_size = map_size_for(self.players());
        }
    }

    /// Whether Defaults can't say Number of sheep and Eats required yet: it
    /// is on and a taken seat is Random, so the wolf count waits for the
    /// match start.
    pub fn defaults_unsure(&self) -> bool {
        self.settings.defaults && self.count(Role::Random) > 0
    }

    /// Seats a person: the first empty seat, or a bot's seat if none is
    /// empty. None when people fill the room. Someone already seated stays
    /// where they are.
    pub fn add_person(&mut self, id: &str, name: &str) -> Option<usize> {
        if let Some(seat) = self.seat_of(id) {
            return Some(seat);
        }
        let free = self.seats.iter().position(|s| s.who == Who::Empty);
        let free = free.or_else(|| self.seats.iter().rposition(|s| s.who == Who::Bot))?;
        self.seats[free] = Seat {
            who: Who::Person {
                id: id.to_string(),
                name: clean_name(name),
                ready: false,
            },
            role: Role::Random,
        };
        self.refill();
        Some(free)
    }

    pub fn remove_person(&mut self, id: &str) {
        if let Some(seat) = self.seat_of(id) {
            self.seats[seat] = Seat::default();
            self.refill();
        }
    }

    pub fn set_name(&mut self, id: &str, name: &str) {
        if let Some(seat) = self.seat_of(id) {
            if let Who::Person { name: n, .. } = &mut self.seats[seat].who {
                *n = clean_name(name);
            }
        }
    }

    pub fn set_ready(&mut self, id: &str, value: bool) {
        if let Some(seat) = self.seat_of(id) {
            if let Who::Person { ready, .. } = &mut self.seats[seat].who {
                *ready = value;
            }
        }
    }

    /// Sets a seat's role. An empty seat has none to set.
    pub fn set_role(&mut self, seat: usize, role: Role) {
        if let Some(s) = self.seats.get_mut(seat) {
            if s.who != Who::Empty {
                s.role = role;
                self.refill();
            }
        }
    }

    /// Puts a bot in an empty seat, or takes one out. A person's seat is
    /// left as it is.
    pub fn set_bot(&mut self, seat: usize, on: bool, role: Role) {
        let Some(s) = self.seats.get_mut(seat) else {
            return;
        };
        match (&s.who, on) {
            (Who::Empty, true) => {
                *s = Seat { who: Who::Bot, role };
            }
            (Who::Bot, false) => *s = Seat::default(),
            _ => {}
        }
        self.refill();
    }

    pub fn set_minutes(&mut self, minutes: i32) {
        self.settings.match_minutes = minutes.clamp(MINUTES.0, MINUTES.1);
    }

    /// The host types the number of sheep: from now on it stays as typed.
    /// Defaults on, it can't be typed.
    pub fn set_sheep(&mut self, sheep: i32) {
        if self.settings.defaults {
            return;
        }
        self.settings.set_sheep(sheep.clamp(SHEEP.0, SHEEP.1));
        self.refill();
    }

    /// The host types the eats required: from now on it stays as typed.
    /// Defaults on, it can't be typed.
    pub fn set_eats(&mut self, eats: i32) {
        if self.settings.defaults {
            return;
        }
        self.settings.set_eats_required(eats.clamp(EATS.0, EATS.1));
        self.refill();
    }

    pub fn set_powerups(&mut self, on: bool) {
        self.settings.powerups = on;
    }

    pub fn set_sudden_death(&mut self, on: bool) {
        self.settings.sudden_death = on;
    }

    /// Defaults on, the Map size follows the players and can't be chosen.
    pub fn set_map_size(&mut self, size: MapSize) {
        if !self.settings.defaults {
            self.settings.map_size = size;
        }
    }

    /// Defaults on refills Number of sheep, Eats required and Map size for
    /// the lobby as it stands; off keeps them as they are, to be typed.
    pub fn set_defaults(&mut self, on: bool) {
        self.settings.set_defaults(on);
        self.refill();
    }

    /// Whether the seats can make at least one of each team, with each
    /// Random seat free to be either.
    pub fn teams_can_form(&self) -> bool {
        let (dogs, wolves, random) = (
            self.count(Role::Sheepdog),
            self.count(Role::Wolf),
            self.count(Role::Random),
        );
        dogs + wolves + random >= 2 && dogs + random >= 1 && wolves + random >= 1
    }

    /// Every person but the host has pressed Ready.
    pub fn everyone_ready(&self) -> bool {
        self.seats.iter().all(|s| match &s.who {
            Who::Person { id, ready, .. } => *ready || *id == self.host,
            _ => true,
        })
    }

    /// What stands in the way of starting, as a status key, or None.
    pub fn start_problem(&self) -> Option<&'static str> {
        if !self.teams_can_form() {
            Some(NEED_BOTH)
        } else if !self.everyone_ready() {
            Some(NEED_READY)
        } else {
            None
        }
    }

    /// The match this lobby starts from `seed`, as `rules::setup` settles it.
    /// The refusal is a status key.
    pub fn start(&self, seed: u64) -> Result<MatchStart, &'static str> {
        if let Some(problem) = self.start_problem() {
            return Err(problem);
        }
        let taken: Vec<(usize, &Seat)> = self
            .seats
            .iter()
            .enumerate()
            .filter(|(_, s)| s.who != Who::Empty)
            .collect();
        let asked: Vec<LobbySeat> = taken
            .iter()
            .map(|(_, s)| LobbySeat {
                choice: s.role.choice(),
                bot: s.who == Who::Bot,
            })
            .collect();
        let setup = start_match(&self.settings, &asked, seed).map_err(|_| NEED_BOTH)?;
        let seats = taken
            .iter()
            .zip(&setup.seats)
            .map(|((k, s), settled)| {
                let (person, name) = match &s.who {
                    Who::Person { id, name, .. } => (id.clone(), name.clone()),
                    _ => (String::new(), String::new()),
                };
                StartSeat {
                    seat: *k as u32,
                    role: settled.role.into(),
                    person,
                    name,
                }
            })
            .collect();
        Ok(MatchStart {
            seed,
            seats,
            settings: setup.settings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A host and `more` other people, every one ready.
    fn people(more: usize) -> Lobby {
        let mut l = Lobby::new("h", "Host");
        for k in 0..more {
            let id = format!("p{k}");
            l.add_person(&id, "P");
            l.set_ready(&id, true);
        }
        l
    }

    #[test]
    fn the_lobby_refills_the_defaults_as_teams_change() {
        let mut l = people(3);
        l.set_role(1, Role::Wolf);
        l.set_role(2, Role::Wolf);
        assert_eq!((l.settings.sheep, l.settings.eats_required), (150, 16));
        l.set_role(2, Role::Sheepdog);
        assert_eq!((l.settings.sheep, l.settings.eats_required), (80, 9));
        l.set_bot(5, true, Role::Wolf);
        assert_eq!((l.settings.sheep, l.settings.eats_required), (150, 16));
        l.set_bot(5, false, Role::Wolf);
        assert_eq!((l.settings.sheep, l.settings.eats_required), (80, 9));
    }

    #[test]
    fn defaults_off_keeps_the_values_to_type_and_on_refills_them() {
        let mut l = people(3);
        l.set_role(0, Role::Sheepdog);
        l.set_role(1, Role::Wolf);
        l.set_role(2, Role::Wolf);
        l.set_role(3, Role::Random);
        l.set_sheep(123);
        l.set_map_size(MapSize::Big);
        // Defaults on: two sure Wolves, 100 + 50 sheep and 8 eats each; four
        // players get a Medium map; typing does nothing
        let s = l.settings;
        assert_eq!((s.sheep, s.eats_required, s.map_size), (150, 16, MapSize::Medium));
        l.set_defaults(false);
        l.set_role(3, Role::Wolf);
        let s = l.settings;
        assert_eq!(
            (s.sheep, s.eats_required, s.map_size),
            (150, 16, MapSize::Medium),
            "kept though a third Wolf sat down"
        );
        l.set_sheep(123);
        l.set_map_size(MapSize::Big);
        let s = l.settings;
        assert_eq!(
            (s.sheep, s.eats_required, s.map_size),
            (123, 16, MapSize::Big),
            "kept as typed"
        );
        l.set_defaults(true);
        // three Wolves: 100 + 50 + 50 sheep, 24 eats
        let s = l.settings;
        assert_eq!((s.sheep, s.eats_required, s.map_size), (200, 24, MapSize::Medium));
    }

    #[test]
    fn defaults_size_the_map_by_the_players_whatever_the_teams() {
        let mut l = Lobby::new("h", "Host");
        l.set_role(0, Role::Wolf);
        assert_eq!(l.settings.map_size, MapSize::Small, "one player");
        for seat in 1..4 {
            l.set_bot(seat, true, Role::Random);
        }
        assert_eq!(l.settings.map_size, MapSize::Medium, "four players");
        l.set_bot(4, true, Role::Sheepdog);
        l.set_bot(5, true, Role::Wolf);
        assert_eq!(l.settings.map_size, MapSize::Big, "six players");
        assert_eq!(l.start(3).unwrap().settings.map_size, MapSize::Big);
        l.set_bot(5, false, Role::Wolf);
        l.set_bot(4, false, Role::Wolf);
        assert_eq!(l.settings.map_size, MapSize::Medium, "back to four");
    }

    #[test]
    fn defaults_are_unsure_while_a_taken_seat_is_random() {
        let mut l = people(1);
        assert!(l.defaults_unsure(), "both seats start Random");
        l.set_role(0, Role::Wolf);
        assert!(l.defaults_unsure(), "one is still Random");
        l.set_role(1, Role::Sheepdog);
        assert!(!l.defaults_unsure());
        l.set_role(1, Role::Random);
        l.set_defaults(false);
        assert!(!l.defaults_unsure(), "typed values are sure");
    }

    #[test]
    fn the_hosts_map_size_goes_with_the_seed() {
        let mut l = people(1);
        l.set_role(0, Role::Sheepdog);
        l.set_role(1, Role::Wolf);
        l.set_defaults(false);
        l.set_map_size(MapSize::Medium);
        let start = l.start(5).unwrap();
        assert_eq!(start.settings.map_size, MapSize::Medium);
        let text = serde_json::to_string(&start).unwrap();
        assert!(text.contains(r#""map_size":"medium""#), "{text}");
        let read: MatchStart = serde_json::from_str(&text).unwrap();
        assert_eq!(read, start);
    }

    #[test]
    fn a_start_needs_one_on_each_team_and_random_seats_count_as_either() {
        let mut l = people(1);
        l.set_role(0, Role::Sheepdog);
        l.set_role(1, Role::Sheepdog);
        assert_eq!(l.start_problem(), Some(NEED_BOTH), "two Sheepdogs");
        assert_eq!(l.start(1).unwrap_err(), NEED_BOTH, "the rules agree");
        l.set_role(0, Role::Wolf);
        l.set_role(1, Role::Wolf);
        assert_eq!(l.start_problem(), Some(NEED_BOTH), "two Wolves");
        l.set_role(1, Role::Sheepdog);
        assert_eq!(l.start_problem(), None, "one of each");
        assert!(l.start(1).is_ok());
        l.set_role(0, Role::Random);
        assert_eq!(l.start_problem(), None, "a Random seat can be the Wolf");
        l.set_role(1, Role::Random);
        assert_eq!(l.start_problem(), None, "two Random seats can be one of each");
        assert_eq!(
            Lobby::new("h", "Host").start_problem(),
            Some(NEED_BOTH),
            "one seat cannot be both teams"
        );
    }

    #[test]
    fn random_seats_settle_with_one_on_each_team_on_every_seed() {
        let mut one_wolf = people(1);
        one_wolf.set_role(0, Role::Wolf);
        let all_random = people(3);
        for seed in 0..200 {
            for lobby in [&one_wolf, &all_random] {
                let m = lobby.start(seed).unwrap();
                let dogs = m.seats.iter().filter(|s| s.role == Role::Sheepdog).count();
                let wolves = m.seats.iter().filter(|s| s.role == Role::Wolf).count();
                assert_eq!(dogs + wolves, m.seats.len(), "seed {seed}: a seat stayed Random");
                assert!(dogs >= 1 && wolves >= 1, "seed {seed}: {dogs} and {wolves}");
            }
            assert_eq!(one_wolf.start(seed).unwrap().seats[1].role, Role::Sheepdog);
        }
    }

    #[test]
    fn a_start_is_the_same_from_the_same_seed() {
        let l = people(3);
        assert_eq!(l.start(42).unwrap(), l.start(42).unwrap());
    }

    #[test]
    fn random_roles_settle_the_defaults_at_match_start() {
        let mut l = people(3);
        l.set_role(0, Role::Wolf);
        l.set_role(1, Role::Wolf);
        l.set_role(2, Role::Random);
        l.set_role(3, Role::Random);
        assert_eq!(
            (l.settings.sheep, l.settings.eats_required),
            (150, 16),
            "Random is not counted yet"
        );
        for seed in 0..50 {
            let m = l.start(seed).unwrap();
            let wolves = m.seats.iter().filter(|s| s.role == Role::Wolf).count() as i32;
            assert_eq!(
                (m.settings.sheep, m.settings.eats_required),
                if wolves == 1 {
                    (80, 9)
                } else {
                    (100 + 50 * (wolves - 1), 8 * wolves)
                }
            );
        }
        l.set_defaults(false);
        l.set_sheep(300);
        l.set_eats(30);
        let m = l.start(9).unwrap();
        assert_eq!(
            (m.settings.sheep, m.settings.eats_required),
            (300, 30),
            "typed by hand"
        );
    }

    #[test]
    fn the_host_starts_only_when_everyone_is_ready_and_needs_no_mark_of_its_own() {
        let mut l = people(2);
        assert_eq!(l.start_problem(), None);
        l.set_ready("p1", false);
        assert_eq!(l.start_problem(), Some(NEED_READY));
        l.set_ready("p1", true);
        assert!(l.start(5).is_ok());
    }

    #[test]
    fn a_bot_sits_in_an_empty_seat_and_makes_way_for_a_person() {
        let mut l = Lobby::new("h", "Host");
        l.set_bot(3, true, Role::Wolf);
        assert_eq!(l.seats[3].who, Who::Bot);
        assert_eq!(l.start_problem(), None, "a bot needs no Ready");
        // a person's seat takes no bot
        l.set_bot(0, true, Role::Wolf);
        assert!(matches!(l.seats[0].who, Who::Person { .. }));
        for seat in 1..MAX_SEATS {
            l.set_bot(seat, true, Role::Sheepdog);
        }
        assert_eq!(
            l.add_person("p", "P"),
            Some(MAX_SEATS - 1),
            "the last bot gives way"
        );
        assert_eq!(l.seats[MAX_SEATS - 1].role, Role::Random);
    }

    #[test]
    fn a_ninth_person_finds_no_seat_and_a_returning_one_keeps_theirs() {
        let mut l = people(MAX_SEATS - 1);
        assert_eq!(l.add_person("late", "Late"), None);
        assert_eq!(l.add_person("p2", "P"), Some(3));
        l.remove_person("p2");
        assert_eq!(l.add_person("late", "Late"), Some(3));
    }

    #[test]
    fn eats_required_stays_within_the_sheep() {
        let mut l = Lobby::new("h", "Host");
        l.set_defaults(false);
        l.set_eats(50);
        l.set_sheep(20);
        assert_eq!(l.settings.eats_required, 20);
    }

    #[test]
    fn the_match_settings_stay_within_their_limits() {
        let mut l = Lobby::new("h", "Host");
        l.set_minutes(0);
        assert_eq!(l.settings.match_minutes, MINUTES.0);
        l.set_minutes(999);
        assert_eq!(l.settings.match_minutes, MINUTES.1);
        l.set_defaults(false);
        l.set_sheep(1);
        assert_eq!(l.settings.sheep, SHEEP.0);
        l.set_sheep(9999);
        assert_eq!(l.settings.sheep, SHEEP.1);
    }

    #[test]
    fn the_brief_s_default_settings_open_the_lobby() {
        let s = Lobby::new("h", "Host").settings;
        assert_eq!((s.match_minutes, s.sheep, s.eats_required), (8, 80, 9));
        assert!(s.powerups, "powerups start on");
        assert!(!s.sudden_death, "sudden death starts off");
    }

    #[test]
    fn a_name_loses_control_characters_and_stops_at_its_limit() {
        assert_eq!(clean_name("  Morag\u{7}  "), "Morag");
        let long = "x".repeat(MAX_NAME_CHARS + 5);
        assert_eq!(clean_name(&long).chars().count(), MAX_NAME_CHARS);
    }

    #[test]
    fn a_role_reads_back_from_its_key() {
        for role in [Role::Sheepdog, Role::Wolf, Role::Random] {
            assert_eq!(Role::parse(role.key()), Some(role));
        }
        assert_eq!(Role::parse("sheep"), None);
    }
}
