//! The tunnel cooldown. A tunnel has two mouths, always the same pair, so an
//! entry takes the far mouth's id and hands it back if the Wolf may go. The
//! cooldown is per player and runs from going in. Tunnels are Wolf-only.

use super::timer::Timer;
use super::tune::{self, secs};
use super::Role;
use crate::fx::Fx;

const COOLDOWN: i32 = secs(tune::TUNNEL_COOLDOWN_SECONDS);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TunnelCooldown {
    wait: Timer,
}

impl TunnelCooldown {
    /// The mouth this player comes out of, or `None` when they can't go in.
    pub fn enter(&mut self, role: Role, far_mouth: u16) -> Option<u16> {
        if role != Role::Wolf || self.wait.running() {
            return None;
        }
        self.wait.start(COOLDOWN);
        Some(far_mouth)
    }

    pub fn tick(&mut self) {
        self.wait.tick();
    }

    /// The HUD's dial: 0 just entered, 1 ready.
    pub fn fraction(&self) -> Fx {
        self.wait.fraction_done(COOLDOWN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx;

    #[test]
    fn a_wolf_comes_out_of_the_far_mouth() {
        assert_eq!(TunnelCooldown::default().enter(Role::Wolf, 9), Some(9));
    }

    #[test]
    fn a_sheepdog_cannot_enter_a_tunnel() {
        let mut cooldown = TunnelCooldown::default();
        assert_eq!(cooldown.enter(Role::Sheepdog, 9), None);
        // and the attempt starts no cooldown
        assert_eq!(cooldown.fraction(), fx::ONE);
    }

    #[test]
    fn a_wolf_waits_thirty_seconds_between_entries() {
        let mut cooldown = TunnelCooldown::default();
        cooldown.enter(Role::Wolf, 9);
        // 30 s at 60 ticks a second is 1800 ticks
        for _ in 0..1799 {
            cooldown.tick();
        }
        assert_eq!(cooldown.enter(Role::Wolf, 4), None);
        cooldown.tick();
        assert_eq!(cooldown.enter(Role::Wolf, 4), Some(4));
    }

    #[test]
    fn the_cooldown_reads_from_zero_to_one_for_the_hud() {
        let mut cooldown = TunnelCooldown::default();
        cooldown.enter(Role::Wolf, 9);
        assert_eq!(cooldown.fraction(), 0);
        // 450 of 1800 ticks is a quarter: 65536 / 4
        for _ in 0..450 {
            cooldown.tick();
        }
        assert_eq!(cooldown.fraction(), 16384);
    }
}
