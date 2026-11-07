//! Mounds. Powerups are unique per team and either team can dig a mound, so a
//! mound hides one powerup for each, and the digger's team says which comes
//! out. A mound is claimed once, by one player, then shows as dug up. A
//! Bone's decoy hides no powerup: a Sheepdog who digs it plays with the bone.
//! Wolves can tell a decoy and don't dig one; Sheepdogs can't tell.
//!
//! A dig takes `DIG_SECONDS` of holding E. Each tick `World` asks `Mound::check`
//! about the mound under the crosshair, gives `Digging::tick` that mound if E
//! is held and the check passes, and calls `Mound::dig` when `Digging` finishes.

use serde::Serialize;

use super::powerups::rows_for;
use super::timer::Timer;
use super::tune::{self, secs, Powerup};
use super::Role;
use crate::fx::{self, Fx};
use crate::rng::Rng;

const DIG: i32 = secs(tune::DIG_SECONDS);
const PLANT: i32 = secs(tune::BONE_PLANT_SECONDS);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Hidden {
    Powerups { wolf: Powerup, sheepdog: Powerup },
    Decoy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Mound {
    hidden: Hidden,
    claimed_by: Option<u8>,
}

/// What a finished dig gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dug {
    Powerup(Powerup),
    /// A decoy: the Sheepdog can do nothing else for this many ticks.
    Decoy {
        stun_ticks: i32,
    },
}

/// Why a dig can't happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DigRefusal {
    DugUp,
    /// A Wolf at a decoy.
    Decoy,
    /// The player must use or discard what they hold first.
    SlotFull,
}

/// How a mound looks to a viewer. It never says what's hidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum MoundLook {
    Fresh,
    Decoy,
    DugUp,
}

impl Mound {
    /// A mound with its two powerups set.
    pub fn buried(wolf: Powerup, sheepdog: Powerup) -> Mound {
        Mound {
            hidden: Hidden::Powerups { wolf, sheepdog },
            claimed_by: None,
        }
    }

    /// A new mound with one powerup drawn for each team, every row as likely.
    pub fn drawn(rng: &mut Rng) -> Mound {
        let wolf = draw(Role::Wolf, rng);
        let sheepdog = draw(Role::Sheepdog, rng);
        Mound::buried(wolf, sheepdog)
    }

    /// The mound a Bone plants.
    pub fn decoy() -> Mound {
        Mound {
            hidden: Hidden::Decoy,
            claimed_by: None,
        }
    }

    pub fn claimed_by(&self) -> Option<u8> {
        self.claimed_by
    }

    /// Whether this player can dig here. `slot_free` is `PowerupSlot::is_free`.
    /// A full slot refuses a real mound and a decoy alike, so it gives neither away.
    pub fn check(&self, role: Role, slot_free: bool) -> Result<(), DigRefusal> {
        if self.claimed_by.is_some() {
            return Err(DigRefusal::DugUp);
        }
        if self.hidden == Hidden::Decoy && role == Role::Wolf {
            return Err(DigRefusal::Decoy);
        }
        if !slot_free {
            return Err(DigRefusal::SlotFull);
        }
        Ok(())
    }

    /// A finished dig: claims the mound for `player`.
    pub fn dig(&mut self, player: u8, role: Role, slot_free: bool) -> Result<Dug, DigRefusal> {
        self.check(role, slot_free)?;
        self.claimed_by = Some(player);
        Ok(match (self.hidden, role) {
            (Hidden::Powerups { wolf, .. }, Role::Wolf) => Dug::Powerup(wolf),
            (Hidden::Powerups { sheepdog, .. }, Role::Sheepdog) => Dug::Powerup(sheepdog),
            (Hidden::Decoy, _) => Dug::Decoy {
                stun_ticks: secs(tune::BONE_STUN_SECONDS),
            },
        })
    }

    /// How this mound looks to a viewer on `viewer`'s team.
    pub fn look(&self, viewer: Role) -> MoundLook {
        if self.claimed_by.is_some() {
            MoundLook::DugUp
        } else if self.hidden == Hidden::Decoy && viewer == Role::Wolf {
            MoundLook::Decoy
        } else {
            MoundLook::Fresh
        }
    }
}

fn draw(role: Role, rng: &mut Rng) -> Powerup {
    let rows = rows_for(role).count() as u32;
    let mut pick = rows_for(role).skip(rng.below(rows) as usize);
    pick.next().expect("a draw below the row count finds a row").kind
}

/// One player's dig under way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Digging {
    mound: u16,
    held: i32,
}

impl Digging {
    /// One tick. `at` is the mound being dug this tick, or `None` when the
    /// player let go, looked away or can't dig it: anything but carrying on
    /// starts the dig over. Returns the mound on the tick the dig finishes.
    pub fn tick(&mut self, at: Option<u16>) -> Option<u16> {
        let Some(mound) = at else {
            self.held = 0;
            return None;
        };
        if mound != self.mound {
            self.mound = mound;
            self.held = 0;
        }
        self.held += 1;
        if self.held < DIG {
            return None;
        }
        self.held = 0;
        Some(mound)
    }

    /// The HUD's progress ring: 0 not digging, 1 done.
    pub fn fraction(&self) -> Fx {
        fx::ratio(self.held, DIG)
    }
}

/// One player's Bone going into the ground: a dig the other way round.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Planting {
    held: i32,
}

impl Planting {
    /// One tick. `going` is false when the player let go or moved, which
    /// starts it over. True on the tick the Bone is planted.
    pub fn tick(&mut self, going: bool) -> bool {
        self.held = if going { self.held + 1 } else { 0 };
        if self.held < PLANT {
            return false;
        }
        self.held = 0;
        true
    }

    /// The HUD's progress bar: 0 not planting, 1 done.
    pub fn fraction(&self) -> Fx {
        fx::ratio(self.held, PLANT)
    }
}

/// How many mounds the map starts with.
pub fn mounds_at_start(powerups_on: bool, players: i32) -> i32 {
    if powerups_on {
        tune::MOUNDS_PER_PLAYER * players
    } else {
        0
    }
}

/// Says when a new mound is due. With powerups off, none ever is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MoundClock {
    wait: Timer,
    every: i32,
}

impl MoundClock {
    pub fn new(powerups_on: bool, players: i32) -> MoundClock {
        let every = secs(tune::NEW_MOUND_PLAYER_SECONDS / players.max(1));
        let mut wait = Timer::default();
        if powerups_on {
            wait.start(every);
        }
        MoundClock { wait, every }
    }

    /// One tick. True on a tick a new mound is due.
    pub fn tick(&mut self) -> bool {
        let due = self.wait.tick();
        if due {
            self.wait.start(self.every);
        }
        due
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::tune::Powerup::{Bone, Howl, Stampede, Zoomies};

    #[test]
    fn planting_a_bone_finishes_on_its_180th_tick_and_going_on_is_what_counts() {
        let mut p = Planting::default();
        for _ in 0..179 {
            assert!(!p.tick(true));
        }
        assert!(!p.tick(false));
        assert_eq!(p.fraction(), 0);
        for _ in 0..179 {
            assert!(!p.tick(true));
        }
        assert!(p.tick(true));
        assert_eq!(p.fraction(), 0);
    }
    use crate::rules::tune::POWERUPS;

    fn mound() -> Mound {
        Mound::buried(Howl, Zoomies)
    }

    #[test]
    fn digging_gives_the_powerup_for_the_diggers_team() {
        assert_eq!(mound().dig(0, Role::Wolf, true), Ok(Dug::Powerup(Howl)));
        assert_eq!(mound().dig(0, Role::Sheepdog, true), Ok(Dug::Powerup(Zoomies)));
    }

    #[test]
    fn a_mound_is_claimed_once_by_one_player() {
        let mut m = mound();
        assert_eq!(m.claimed_by(), None);
        assert!(m.dig(3, Role::Wolf, true).is_ok());
        assert_eq!(m.dig(5, Role::Sheepdog, true), Err(DigRefusal::DugUp));
        assert_eq!(m.claimed_by(), Some(3));
    }

    #[test]
    fn a_full_slot_refuses_the_dig_and_leaves_the_mound() {
        let mut m = mound();
        assert_eq!(m.dig(0, Role::Wolf, false), Err(DigRefusal::SlotFull));
        assert_eq!(m.look(Role::Wolf), MoundLook::Fresh);
        assert!(m.dig(0, Role::Wolf, true).is_ok());
    }

    #[test]
    fn a_decoy_stops_the_sheepdog_who_digs_it_for_ten_seconds() {
        let mut m = Mound::decoy();
        // 10 s at 60 ticks a second is 600 ticks
        let dug = Ok(Dug::Decoy { stun_ticks: 600 });
        assert_eq!(m.dig(1, Role::Sheepdog, true), dug);
        assert_eq!(m.look(Role::Sheepdog), MoundLook::DugUp);
    }

    #[test]
    fn a_wolf_is_refused_at_a_decoy_and_leaves_it_in_place() {
        let mut m = Mound::decoy();
        assert_eq!(m.dig(0, Role::Wolf, true), Err(DigRefusal::Decoy));
        assert_eq!(m.claimed_by(), None);
    }

    #[test]
    fn a_full_slot_refuses_a_decoy_like_a_real_mound() {
        let refused = Err(DigRefusal::SlotFull);
        assert_eq!(Mound::decoy().check(Role::Sheepdog, false), refused);
        assert_eq!(mound().check(Role::Sheepdog, false), refused);
    }

    #[test]
    fn sheepdogs_cannot_tell_a_decoy_from_a_mound_and_wolves_can() {
        assert_eq!(Mound::decoy().look(Role::Sheepdog), mound().look(Role::Sheepdog));
        assert_eq!(Mound::decoy().look(Role::Wolf), MoundLook::Decoy);
        assert_eq!(mound().look(Role::Wolf), MoundLook::Fresh);
    }

    #[test]
    fn a_new_mound_draws_one_powerup_for_each_team_from_the_seed() {
        // Worked out with a separate SplitMix64: below(6) gives 5, then 2.
        // The sixth Wolf row is Stampede and the third Sheepdog row is Zoomies.
        let drawn = Mound::drawn(&mut Rng::new(0));
        assert_eq!(drawn, Mound::buried(Stampede, Zoomies));
        assert_ne!(drawn, Mound::buried(Bone, Zoomies));
    }

    #[test]
    fn every_powerup_a_team_has_can_come_out_of_a_mound() {
        // 600 draws at equal odds of one in six is about 100 of each
        let mut rng = Rng::new(7);
        let mut seen = [0u32; POWERUPS.len()];
        for _ in 0..600 {
            let mut mound = Mound::drawn(&mut rng);
            if let Ok(Dug::Powerup(kind)) = mound.dig(0, Role::Wolf, true) {
                seen[kind as usize] += 1;
            }
        }
        for row in rows_for(Role::Wolf) {
            assert!(seen[row.kind as usize] > 60, "{:?}: {seen:?}", row.kind);
        }
    }

    #[test]
    fn a_dig_takes_two_seconds_of_holding() {
        let mut digging = Digging::default();
        // 2 s at 60 ticks a second is 120 ticks
        for _ in 0..119 {
            assert_eq!(digging.tick(Some(7)), None);
        }
        assert_eq!(digging.tick(Some(7)), Some(7));
    }

    #[test]
    fn letting_go_starts_the_dig_over() {
        let mut digging = Digging::default();
        for _ in 0..119 {
            digging.tick(Some(7));
        }
        digging.tick(None);
        for _ in 0..119 {
            assert_eq!(digging.tick(Some(7)), None);
        }
        assert_eq!(digging.tick(Some(7)), Some(7));
    }

    #[test]
    fn moving_to_another_mound_starts_the_dig_over() {
        let mut digging = Digging::default();
        for _ in 0..119 {
            digging.tick(Some(7));
        }
        assert_eq!(digging.tick(Some(8)), None);
        assert_eq!(digging.fraction(), fx::ratio(1, 120));
    }

    #[test]
    fn the_dig_reads_from_zero_to_nearly_one_for_the_hud() {
        let mut digging = Digging::default();
        assert_eq!(digging.fraction(), 0);
        for _ in 0..30 {
            digging.tick(Some(7));
        }
        // 30 of 120 ticks is a quarter: 65536 / 4
        assert_eq!(digging.fraction(), 16384);
    }

    #[test]
    fn a_finished_dig_starts_from_nothing() {
        let mut digging = Digging::default();
        for _ in 0..120 {
            digging.tick(Some(7));
        }
        assert_eq!(digging.fraction(), 0);
    }

    #[test]
    fn twelve_mounds_start_the_match_unless_powerups_are_off() {
        assert_eq!(mounds_at_start(true, 2), 4);
        assert_eq!(mounds_at_start(true, 8), 16);
        assert_eq!(mounds_at_start(false, 8), 0);
    }

    #[test]
    fn a_new_mound_comes_due_each_minute() {
        let mut clock = MoundClock::new(true, 4);
        // 60 s at 60 ticks a second is 3600 ticks, twice over
        for _ in 0..2 {
            for _ in 0..3599 {
                assert!(!clock.tick());
            }
            assert!(clock.tick());
        }
    }

    #[test]
    fn no_new_mound_comes_with_powerups_off() {
        let mut clock = MoundClock::new(false, 4);
        assert!((0..4000).all(|_| !clock.tick()));
    }
}
