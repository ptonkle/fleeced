//! A Sheepdog's Lunge. A click commits it. `World` moves the Sheepdog up to
//! `tune::LUNGE_REACH` and says when it reaches a Wolf; a Lunge that runs out
//! with no hit is a miss, and a miss means no Lunge for a while. A hit carries
//! no wait, since the brief puts the wait on a miss alone.

use super::timer::Timer;
use super::tune::{self, secs};
use crate::fx::Fx;

const MISS_WAIT: i32 = secs(tune::LUNGE_MISS_SECONDS);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Lunge {
    going: Timer,
    miss_wait: Timer,
}

impl Lunge {
    pub fn ready(&self) -> bool {
        !self.going.running() && !self.miss_wait.running()
    }

    pub fn lunging(&self) -> bool {
        self.going.running()
    }

    /// A left click. True when it commits a Lunge.
    pub fn click(&mut self) -> bool {
        if !self.ready() {
            return false;
        }
        self.going.start(tune::LUNGE_TICKS);
        true
    }

    /// `World` found a Wolf in reach of this Lunge. It ends with no wait.
    pub fn hit(&mut self) {
        self.going.stop();
    }

    /// One tick. True on the tick a Lunge ends in a miss, when nearby sheep
    /// scatter.
    pub fn tick(&mut self) -> bool {
        self.miss_wait.tick();
        let missed = self.going.tick();
        if missed {
            self.miss_wait.start(MISS_WAIT);
        }
        missed
    }

    /// The wait after a miss, for the HUD: 0 just missed, 1 ready.
    pub fn wait_fraction(&self) -> Fx {
        self.miss_wait.fraction_done(MISS_WAIT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx;

    /// A Lunge run out to its miss.
    fn missed() -> Lunge {
        let mut lunge = Lunge::default();
        lunge.click();
        while !lunge.tick() {}
        lunge
    }

    #[test]
    fn a_lunge_reaches_ten_point_eight_meters() {
        // 54 * 65536 / 5, rounded down
        assert_eq!(tune::LUNGE_REACH, 707_788);
    }

    #[test]
    fn a_click_commits_a_lunge_and_a_second_click_does_nothing() {
        let mut lunge = Lunge::default();
        assert!(lunge.ready());
        assert!(lunge.click());
        assert!(lunge.lunging());
        assert!(!lunge.click());
    }

    #[test]
    fn a_hit_ends_the_lunge_with_no_wait() {
        let mut lunge = Lunge::default();
        lunge.click();
        lunge.tick();
        lunge.hit();
        assert!(!lunge.lunging());
        assert!(lunge.ready());
        // and it doesn't turn into a miss on a later tick
        assert!(!lunge.tick());
    }

    #[test]
    fn a_lunge_that_runs_out_is_a_miss_and_says_so_once() {
        let mut lunge = Lunge::default();
        lunge.click();
        for _ in 0..tune::LUNGE_TICKS - 1 {
            assert!(!lunge.tick());
            assert!(lunge.lunging());
        }
        assert!(lunge.tick());
        assert!(!lunge.lunging());
        assert!(!lunge.tick());
    }

    #[test]
    fn a_miss_means_no_lunge_for_four_seconds() {
        let mut lunge = missed();
        // 4 s at 60 ticks a second is 240 ticks
        for _ in 0..239 {
            lunge.tick();
        }
        assert!(!lunge.click());
        lunge.tick();
        assert!(lunge.click());
    }

    #[test]
    fn a_hit_outside_a_lunge_does_not_lift_the_wait() {
        let mut lunge = missed();
        lunge.hit();
        assert!(!lunge.ready());
    }

    #[test]
    fn the_wait_after_a_miss_reads_from_zero_to_one() {
        let mut lunge = missed();
        assert_eq!(lunge.wait_fraction(), 0);
        // 60 of 240 ticks is a quarter: 65536 / 4
        for _ in 0..60 {
            lunge.tick();
        }
        assert_eq!(lunge.wait_fraction(), 16384);
        for _ in 0..180 {
            lunge.tick();
        }
        assert_eq!(lunge.wait_fraction(), fx::ONE);
    }
}
