//! A Sheepdog's bark cooldown. `World` scares the sheep.

use super::timer::Timer;
use super::tune::{self, secs};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BarkCooldown {
    wait: Timer,
}

impl BarkCooldown {
    /// A bark. True when it goes out; false while the cooldown runs.
    pub fn bark(&mut self) -> bool {
        if self.wait.running() {
            return false;
        }
        self.wait.start(secs(tune::BARK_COOLDOWN_SECONDS));
        true
    }

    pub fn tick(&mut self) {
        self.wait.tick();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sheepdog_barks_every_three_seconds() {
        let mut cooldown = BarkCooldown::default();
        assert!(cooldown.bark());
        // 3 s at 60 ticks a second is 180 ticks
        for _ in 0..179 {
            cooldown.tick();
        }
        assert!(!cooldown.bark());
        cooldown.tick();
        assert!(cooldown.bark());
    }
}
