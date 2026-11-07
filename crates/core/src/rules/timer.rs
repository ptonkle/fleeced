//! A countdown in ticks, the piece most rules share.

use crate::fx::{self, Fx};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Timer {
    left: i32,
}

impl Timer {
    /// A timer already counting down.
    pub fn started(ticks: i32) -> Timer {
        Timer { left: ticks }
    }

    pub fn start(&mut self, ticks: i32) {
        self.left = ticks;
    }

    pub fn stop(&mut self) {
        self.left = 0;
    }

    /// One tick down. True on the tick it reaches zero, and at no other.
    pub fn tick(&mut self) -> bool {
        if self.left == 0 {
            return false;
        }
        self.left -= 1;
        self.left == 0
    }

    pub fn running(&self) -> bool {
        self.left > 0
    }

    /// How much of `total` ticks has passed: 0 just started, 1 when stopped.
    pub fn fraction_done(&self, total: i32) -> Fx {
        fx::ratio(total - self.left, total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_timer_runs_for_exactly_its_ticks_and_fires_once() {
        let mut t = Timer::started(3);
        assert!(!t.tick());
        assert!(!t.tick());
        assert!(t.running());
        assert!(t.tick());
        assert!(!t.running());
        assert!(!t.tick());
    }

    #[test]
    fn a_timer_reads_how_far_along_it_is() {
        let mut t = Timer::started(4);
        assert_eq!(t.fraction_done(4), 0);
        t.tick();
        // one of four: 65536 / 4
        assert_eq!(t.fraction_done(4), 16384);
        t.stop();
        assert_eq!(t.fraction_done(4), fx::ONE);
    }
}
