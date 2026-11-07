//! A Wolf's eat dial, and the moment the Wolf is locked in place by an eat.
//! The cooldown is flat, whatever the wolf count (FL-3), so nothing here takes
//! one. The dial starts full: every Wolf's first eat waits out a whole cooldown.

use super::tune::{self, secs};
use crate::fx::{self, Fx};

const COOLDOWN: i32 = secs(tune::EAT_COOLDOWN_SECONDS);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EatDial {
    /// Ticks of cooldown left to run.
    left: i32,
    /// The tick the Wolf can move again.
    lock_ends: i32,
}

impl Default for EatDial {
    fn default() -> EatDial {
        EatDial {
            left: COOLDOWN,
            lock_ends: 0,
        }
    }
}

impl EatDial {
    pub fn ready(&self) -> bool {
        self.left == 0
    }

    /// Eats at tick `now`: the dial starts over and the Wolf is locked in
    /// place. False, with nothing changed, while the dial is coming back.
    pub fn eat(&mut self, now: i32) -> bool {
        if !self.ready() {
            return false;
        }
        self.left = COOLDOWN;
        self.lock_ends = now + secs(tune::EAT_LOCK_SECONDS);
        true
    }

    /// One tick. Super appetite running doubles the dial's speed. `World`
    /// calls this only while the Wolf is alive: a dial running through the
    /// wait to return would make a kill cost next to nothing (R1.617).
    pub fn tick(&mut self, super_appetite: bool) {
        let step = if super_appetite {
            tune::SUPER_APPETITE_SPEED
        } else {
            1
        };
        self.left = (self.left - step).max(0);
    }

    /// Whether the Wolf is locked in place at tick `now`.
    pub fn locked(&self, now: i32) -> bool {
        now < self.lock_ends
    }

    /// The tick the Wolf can move again.
    pub fn lock_ends(&self) -> i32 {
        self.lock_ends
    }

    /// The whole seconds the dial has yet to run, rounded up.
    pub fn seconds_left(&self) -> i32 {
        (self.left + secs(1) - 1) / secs(1)
    }

    /// The HUD's countdown dial: 0 just eaten, 1 ready.
    pub fn fraction(&self) -> Fx {
        fx::ratio(COOLDOWN - self.left, COOLDOWN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ticked(dial: &mut EatDial, ticks: i32, super_appetite: bool) {
        for _ in 0..ticks {
            dial.tick(super_appetite);
        }
    }

    #[test]
    fn the_first_eat_waits_out_a_whole_cooldown() {
        let mut dial = EatDial::default();
        // 20 s at 60 ticks a second is 1200 ticks
        ticked(&mut dial, 1199, false);
        assert!(!dial.ready());
        assert!(!dial.eat(1199));
        dial.tick(false);
        assert!(dial.ready());
        assert!(dial.eat(1200));
    }

    #[test]
    fn the_dial_takes_twenty_seconds_to_come_back() {
        let mut dial = EatDial::default();
        ticked(&mut dial, 1200, false);
        dial.eat(1200);
        ticked(&mut dial, 1199, false);
        assert!(!dial.ready());
        dial.tick(false);
        assert!(dial.ready());
    }

    #[test]
    fn super_appetite_brings_the_dial_back_at_double_speed() {
        let mut dial = EatDial::default();
        // 1200 ticks of dial at two a tick is 600 ticks
        ticked(&mut dial, 599, true);
        assert!(!dial.ready());
        dial.tick(true);
        assert!(dial.ready());
    }

    #[test]
    fn super_appetite_for_part_of_a_cooldown_counts_for_that_part() {
        let mut dial = EatDial::default();
        // 300 ticks at double speed cover 600, leaving 600 at single speed
        ticked(&mut dial, 300, true);
        ticked(&mut dial, 599, false);
        assert!(!dial.ready());
        dial.tick(false);
        assert!(dial.ready());
    }

    #[test]
    fn the_dial_reads_from_zero_to_one_for_the_hud() {
        let mut dial = EatDial::default();
        assert_eq!(dial.fraction(), 0);
        // 300 of 1200 ticks is a quarter: 65536 / 4
        ticked(&mut dial, 300, false);
        assert_eq!(dial.fraction(), 16384);
        ticked(&mut dial, 900, false);
        assert_eq!(dial.fraction(), fx::ONE);
    }

    #[test]
    fn a_refused_eat_leaves_the_dial_and_the_wolf_alone() {
        let mut dial = EatDial::default();
        ticked(&mut dial, 300, false);
        assert!(!dial.eat(300));
        assert_eq!(dial.fraction(), 16384);
        assert!(!dial.locked(300));
    }

    #[test]
    fn eating_locks_the_wolf_in_place_for_one_second() {
        let mut dial = EatDial::default();
        ticked(&mut dial, 1200, false);
        assert!(!dial.locked(1500));
        dial.eat(1500);
        // 1 s is 60 ticks from tick 1500
        assert_eq!(dial.lock_ends(), 1560);
        assert!(dial.locked(1500));
        assert!(dial.locked(1559));
        assert!(!dial.locked(1560));
    }
}
