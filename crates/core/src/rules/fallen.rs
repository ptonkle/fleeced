//! A Wolf just killed. It lies where it died for a while, for all in sight to
//! see, however it was disguised, and the kill shows on the Sheepdogs' screens
//! for less. `Match` keeps a list of these, so a rollback undoes one like any
//! other state. What the rule doesn't touch: the Wolf's own wait to return,
//! which is `death.rs`.

use super::tune;
use super::Role;
use crate::fx::{self, Fx};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fallen {
    pub x: Fx,
    pub z: Fx,
    pub yaw: u16,
    /// The seat of the player who made the kill.
    pub killer: u8,
    /// Who died: a Wolf to a Lunge, or a Sheepdog to a Frenzy.
    pub role: Role,
    age: i32,
}

impl Fallen {
    pub fn new(x: Fx, z: Fx, yaw: u16, killer: u8) -> Fallen {
        Fallen {
            x,
            z,
            yaw,
            killer,
            role: Role::Wolf,
            age: 0,
        }
    }

    /// A Sheepdog killed by a Wolf in a Frenzy.
    pub fn sheepdog(x: Fx, z: Fx, yaw: u16, killer: u8) -> Fallen {
        Fallen {
            role: Role::Sheepdog,
            ..Fallen::new(x, z, yaw, killer)
        }
    }

    /// Ticks since the kill.
    pub fn age(&self) -> i32 {
        self.age
    }

    /// One tick older. True once the body is gone.
    pub fn tick(&mut self) -> bool {
        self.age += 1;
        self.age >= tune::KILL_BODY_TICKS
    }

    /// Whether the Sheepdogs' screens still show this kill: a Wolf's alone.
    pub fn news(&self) -> bool {
        self.role == Role::Wolf && self.age < tune::KILL_NEWS_TICKS
    }

    /// The killer's flash: 1 at the kill, fading to 0.
    pub fn flash(&self) -> Fx {
        fx::ratio((tune::KILL_FLASH_TICKS - self.age).max(0), tune::KILL_FLASH_TICKS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aged(ticks: i32) -> Fallen {
        let mut fallen = Fallen::new(0, 0, 0, 1);
        for _ in 0..ticks {
            fallen.tick();
        }
        fallen
    }

    #[test]
    fn a_body_lies_for_four_seconds() {
        // 4 s at 60 ticks a second is 240 ticks
        let mut fallen = aged(239);
        assert!(fallen.age() == 239);
        assert!(fallen.tick());
    }

    #[test]
    fn a_kill_is_news_for_two_seconds() {
        // 2 s at 60 ticks a second is 120 ticks
        assert!(aged(119).news());
        assert!(!aged(120).news());
    }

    #[test]
    fn the_flash_fades_from_one_to_nothing_in_a_quarter_second() {
        assert_eq!(aged(0).flash(), fx::ONE);
        // 15 ticks of 15, then 5 left of 15 is a third of 65536, rounded down
        assert_eq!(aged(10).flash(), 21845);
        assert_eq!(aged(15).flash(), 0);
        assert_eq!(aged(100).flash(), 0);
    }
}
