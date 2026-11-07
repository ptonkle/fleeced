//! A Bell on a sheep. It hangs silent until a Wolf first comes within reach,
//! cloaked or suited all the same and with no eating needed. Then it rings
//! for everyone for a while, wherever the Wolf goes next, and is used up. It
//! doesn't re-arm. `World` hangs one when a Sheepdog uses a Bell, feeds it the
//! distance each tick, and drops it once it's used up or its sheep is eaten or
//! reaches the Fold. Who sees a worn Bell is in `information.rs`.

use super::timer::Timer;
use super::tune::{self, secs};
use crate::fx::Fx;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Bell {
    sheep: u16,
    set_off: bool,
    ring: Timer,
}

impl Bell {
    pub fn hang(sheep: u16) -> Bell {
        Bell {
            sheep,
            set_off: false,
            ring: Timer::default(),
        }
    }

    /// The sheep wearing it.
    pub fn sheep(&self) -> u16 {
        self.sheep
    }

    pub fn ringing(&self) -> bool {
        self.ring.running()
    }

    /// It has rung, and has nothing left.
    pub fn used_up(&self) -> bool {
        self.set_off && !self.ring.running()
    }

    /// One tick. `nearest_wolf` is the distance in meters from the sheep to
    /// the nearest Wolf on the map, or `None` with no Wolf on it.
    pub fn tick(&mut self, nearest_wolf: Option<Fx>) {
        if self.set_off {
            self.ring.tick();
        } else if nearest_wolf.is_some_and(|d| d <= tune::BELL_REACH) {
            self.set_off = true;
            self.ring.start(secs(tune::BELL_RING_SECONDS));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx;

    const WOLF_AT_REACH: Option<Fx> = Some(fx::from_int(20));

    #[test]
    fn a_bell_hangs_silent_on_its_sheep() {
        let bell = Bell::hang(42);
        assert_eq!(bell.sheep(), 42);
        assert!(!bell.ringing());
        assert!(!bell.used_up());
    }

    #[test]
    fn a_bell_is_set_off_by_a_wolf_within_twenty_meters() {
        let mut bell = Bell::hang(42);
        bell.tick(WOLF_AT_REACH);
        assert!(bell.ringing());
    }

    #[test]
    fn a_wolf_past_twenty_meters_sets_off_nothing() {
        let mut bell = Bell::hang(42);
        bell.tick(Some(fx::from_int(20) + 1));
        assert!(!bell.ringing());
    }

    #[test]
    fn a_bell_with_no_wolf_on_the_map_stays_silent() {
        let mut bell = Bell::hang(42);
        bell.tick(None);
        assert!(!bell.ringing());
    }

    #[test]
    fn a_bell_rings_for_ten_seconds_after_the_wolf_is_gone() {
        let mut bell = Bell::hang(42);
        bell.tick(WOLF_AT_REACH);
        // 10 s at 60 ticks a second is 600 ticks
        for _ in 0..599 {
            bell.tick(None);
        }
        assert!(bell.ringing());
        bell.tick(None);
        assert!(!bell.ringing());
    }

    #[test]
    fn a_rung_bell_is_used_up_and_does_not_re_arm() {
        let mut bell = Bell::hang(42);
        bell.tick(WOLF_AT_REACH);
        for _ in 0..600 {
            bell.tick(None);
        }
        assert!(bell.used_up());
        bell.tick(WOLF_AT_REACH);
        assert!(!bell.ringing());
    }

    #[test]
    fn a_wolf_staying_close_does_not_stretch_the_ringing() {
        let mut bell = Bell::hang(42);
        for _ in 0..601 {
            bell.tick(WOLF_AT_REACH);
        }
        assert!(!bell.ringing());
    }
}
