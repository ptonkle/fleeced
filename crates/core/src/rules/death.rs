//! A killed Wolf. With Sudden death off it returns after a wait; `World`
//! picks the tunnel mouth. With it on, the Wolf stays out and spectates.

use super::timer::Timer;
use super::tune::{self, secs};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Life {
    #[default]
    Alive,
    Returning(Timer),
    Out,
}

impl Life {
    pub fn alive(&self) -> bool {
        *self == Life::Alive
    }

    /// Stayed out under Sudden death, and spectating.
    pub fn spectating(&self) -> bool {
        *self == Life::Out
    }

    /// Whole seconds until a killed Wolf returns, or `None` when it isn't
    /// waiting to.
    pub fn back_in(&self) -> Option<i32> {
        match self {
            Life::Returning(wait) => Some(wait.seconds_left()),
            _ => None,
        }
    }

    /// A kill. A Wolf already dead stays as it is.
    pub fn kill(&mut self, sudden_death: bool) {
        if !self.alive() {
            return;
        }
        *self = if sudden_death {
            Life::Out
        } else {
            Life::Returning(Timer::started(secs(tune::WOLF_RETURN_SECONDS)))
        };
    }

    /// One tick. True on the tick a Wolf returns.
    pub fn tick(&mut self) -> bool {
        if let Life::Returning(wait) = self {
            if wait.tick() {
                *self = Life::Alive;
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_killed_wolf_counts_whole_seconds_down_to_its_return() {
        let mut life = Life::default();
        assert_eq!(life.back_in(), None);
        life.kill(false);
        assert_eq!(life.back_in(), Some(10));
        // one tick in, 9 s and 59 ticks are left: still 10 rounded up
        life.tick();
        assert_eq!(life.back_in(), Some(10));
        for _ in 0..60 {
            life.tick();
        }
        assert_eq!(life.back_in(), Some(9));
        let mut out = Life::default();
        out.kill(true);
        assert_eq!(out.back_in(), None);
    }

    #[test]
    fn a_killed_wolf_returns_after_ten_seconds() {
        let mut life = Life::default();
        life.kill(false);
        assert!(!life.alive());
        assert!(!life.spectating());
        // 10 s at 60 ticks a second is 600 ticks
        for _ in 0..599 {
            assert!(!life.tick());
        }
        assert!(life.tick());
        assert!(life.alive());
    }

    #[test]
    fn under_sudden_death_a_killed_wolf_stays_out_and_spectates() {
        let mut life = Life::default();
        life.kill(true);
        assert!(life.spectating());
        assert!((0..4000).all(|_| !life.tick()));
        assert!(!life.alive());
    }

    #[test]
    fn a_second_kill_does_not_restart_the_wait() {
        let mut life = Life::default();
        life.kill(false);
        for _ in 0..599 {
            life.tick();
        }
        life.kill(false);
        assert!(life.tick());
    }
}
