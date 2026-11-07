//! The match rules: small pure state machines, one to a file, tested
//! directly. None of them knows `World`. Each holds plain values and takes
//! plain values and a tick, so `game::Match` asks them what the rule says and
//! does the moving itself. Every balance number is in `tune.rs`.
//!
//! | File | Rule |
//! | --- | --- |
//! | `setup.rs` | settings, seats, Random roles, lobby defaults, the Fold target |
//! | `outcome.rs` | who has won |
//! | `eat.rs` | the eat dial, and the moment a Wolf is locked in place |
//! | `lunge.rs` | a Sheepdog's Lunge and the wait after a miss |
//! | `bark.rs` | a Sheepdog's bark cooldown |
//! | `powerups.rs` | the slot: hold one, activate, discard, timers and meters |
//! | `night.rs` | Howl's night and its change of light |
//! | `bell.rs` | a Bell on a sheep |
//! | `mounds.rs` | mounds, digging, and new mounds through the match |
//! | `tunnels.rs` | the tunnel cooldown |
//! | `death.rs` | a killed Wolf: back after a wait, or out |
//! | `fallen.rs` | a killed Wolf's body, and how long the kill shows |
//! | `carcasses.rs` | carcasses found by the Sheepdogs |
//! | `information.rs` | what each viewer sees |
//! | `timer.rs` | the countdown most of these share |

pub mod bark;
pub mod bell;
pub mod carcasses;
pub mod death;
pub mod eat;
pub mod fallen;
pub mod information;
pub mod lunge;
pub mod mounds;
pub mod night;
pub mod outcome;
pub mod powerups;
pub mod setup;
pub mod timer;
pub mod tune;
pub mod tunnels;

use serde::{Deserialize, Serialize};

/// A player's team.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Sheepdog,
    Wolf,
}
