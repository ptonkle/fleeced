//! Howl's night. It lasts `HOWL_NIGHT_SECONDS` from the howl, the first
//! `HOWL_CHANGE_SECONDS` of that being the sun going down; then day takes the
//! same change to come back. A howl during a night starts its seconds over,
//! and the light carries on from where it was, so there is no jump.

use super::tune::{self, secs};
use crate::fx::{self, Fx};

const CHANGE: i32 = secs(tune::HOWL_CHANGE_SECONDS);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Night {
    /// Ticks of night left.
    left: i32,
    /// How far the light has changed, in ticks of `CHANGE`.
    dusk: i32,
}

impl Night {
    pub fn howl(&mut self) {
        self.left = secs(tune::HOWL_NIGHT_SECONDS);
    }

    pub fn tick(&mut self) {
        if self.left > 0 {
            self.left -= 1;
            self.dusk = (self.dusk + 1).min(CHANGE);
        } else {
            self.dusk = (self.dusk - 1).max(0);
        }
    }

    /// 0 in full day, 1 in full night.
    pub fn darkness(&self) -> Fx {
        fx::ratio(self.dusk, CHANGE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ticked(night: &mut Night, ticks: i32) {
        for _ in 0..ticks {
            night.tick();
        }
    }

    #[test]
    fn day_is_full_light_until_a_howl() {
        let mut night = Night::default();
        ticked(&mut night, 500);
        assert_eq!(night.darkness(), 0);
    }

    #[test]
    fn the_sun_goes_down_over_three_seconds() {
        let mut night = Night::default();
        night.howl();
        assert_eq!(night.darkness(), 0);
        // 90 of 180 ticks is halfway
        ticked(&mut night, 90);
        assert_eq!(night.darkness(), fx::HALF);
        ticked(&mut night, 90);
        assert_eq!(night.darkness(), fx::ONE);
    }

    #[test]
    fn night_holds_for_sixty_seconds_then_day_comes_back_over_three() {
        let mut night = Night::default();
        night.howl();
        // 60 s is 3600 ticks
        ticked(&mut night, 3600);
        assert_eq!(night.darkness(), fx::ONE);
        ticked(&mut night, 90);
        assert_eq!(night.darkness(), fx::HALF);
        ticked(&mut night, 90);
        assert_eq!(night.darkness(), 0);
    }

    #[test]
    fn a_second_howl_starts_the_sixty_seconds_over() {
        let mut night = Night::default();
        night.howl();
        ticked(&mut night, 3000);
        night.howl();
        ticked(&mut night, 3600);
        assert_eq!(night.darkness(), fx::ONE);
        night.tick();
        assert!(night.darkness() < fx::ONE);
    }

    #[test]
    fn a_howl_as_day_comes_back_turns_the_light_around_from_where_it_is() {
        let mut night = Night::default();
        night.howl();
        ticked(&mut night, 3690);
        assert_eq!(night.darkness(), fx::HALF);
        night.howl();
        ticked(&mut night, 45);
        // 135 of 180 ticks is three quarters: 49152 / 65536
        assert_eq!(night.darkness(), 49152);
    }
}
