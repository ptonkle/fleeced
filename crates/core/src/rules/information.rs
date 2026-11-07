//! What each viewer sees. One pure function, `view`, from the viewer's team and
//! the match to everything one-sided on that player's screen (FL-7). The page
//! draws a `View` and is given nothing else of these, so what a team isn't to
//! know is left out of its `View` altogether.
//!
//! - Outlines: Sheepdogs see Sheepdogs. Wolves see Wolves and, behind
//!   `tune::WOLVES_SEE_DOGS`, Sheepdogs.
//! - Counts: the eats and the count in the Fold reach everyone.
//! - A kill: Sheepdogs hear of one for a moment, wherever they are. Wolves
//!   hear nothing: a Wolf's own outline going is how they know.
//! - Bells: Sheepdogs see which sheep wears one until it's used up, with an
//!   outline. Wolves
//!   get no sign of a worn Bell: a ringing one reaches everyone, as a sound.
//! - Mounds: a Bone's decoy looks like any mound to Sheepdogs.
//! - Sight: how far a player sees, by team and by the light.

use serde::Serialize;

use super::bell::Bell;
use super::fallen::Fallen;
use super::mounds::{Mound, MoundLook};
use super::outcome::Standing;
use super::tune;
use super::Role;
use crate::fx::{self, Fx};

/// The match, as far as `view` needs it.
#[derive(Debug, Clone, Copy)]
pub struct Facts<'a> {
    pub standing: &'a Standing,
    pub bells: &'a [Bell],
    pub fallen: &'a [Fallen],
    pub mounds: &'a [Mound],
    /// `Night::darkness`.
    pub darkness: Fx,
    /// This viewer looks through Dognoculars.
    pub scoped: bool,
}

/// What one viewer sees. A field that isn't for this viewer is left out of
/// the JSON, so the page has nothing to leak.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct View {
    pub outline_sheepdogs: bool,
    pub outline_wolves: bool,
    pub eats: i32,
    pub in_fold: i32,
    /// A Sheepdog just killed a Wolf: this viewer's team hears of it.
    pub wolf_killed: bool,
    /// The sheep wearing a Bell not yet used up, by sheep id.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub worn_bells: Vec<u16>,
    /// The sheep whose Bell is ringing now: where to play the sound from.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ringing_bells: Vec<u16>,
    /// Each mound's look, in mound order.
    pub mounds: Vec<MoundLook>,
    /// How far this viewer sees, in meters.
    pub sight_range: Fx,
    /// How much light this viewer has, 1 by day. A team's night.
    pub light: Fx,
}

fn outline_rule(viewer: Role, target: Role, wolves_see_dogs: bool) -> bool {
    match (viewer, target) {
        (Role::Sheepdog, Role::Sheepdog) | (Role::Wolf, Role::Wolf) => true,
        (Role::Wolf, Role::Sheepdog) => wolves_see_dogs,
        (Role::Sheepdog, Role::Wolf) => false,
    }
}

/// Whether a viewer on one team sees the outline of a player on another.
pub fn sees_outline(viewer: Role, target: Role) -> bool {
    outline_rule(viewer, target, tune::WOLVES_SEE_DOGS)
}

/// How far a team sees at this darkness, moving evenly from the day's range
/// to the night's as the light changes.
pub fn sight_range(role: Role, darkness: Fx) -> Fx {
    let (day, night) = match role {
        Role::Sheepdog => (tune::SIGHT_SHEEPDOG_DAY, tune::SIGHT_SHEEPDOG_NIGHT),
        Role::Wolf => (tune::SIGHT_WOLF_DAY, tune::SIGHT_WOLF_NIGHT),
    };
    day + fx::mul(night - day, darkness)
}

/// How much of the light a team has at this darkness, 1 by day.
pub fn night_light(role: Role, darkness: Fx) -> Fx {
    let night = match role {
        Role::Sheepdog => tune::NIGHT_LIGHT_SHEEPDOG,
        Role::Wolf => tune::NIGHT_LIGHT_WOLF,
    };
    fx::ONE + fx::mul(fx::ratio(night, 100) - fx::ONE, darkness)
}

/// Whether a point is within a viewer's sight. What's past it isn't on that
/// viewer's page: that's what keeps the count of sheep from Sheepdogs.
pub fn in_sight(range: Fx, from: (Fx, Fx), to: (Fx, Fx)) -> bool {
    fx::length(to.0 - from.0, to.1 - from.1) <= range
}

/// Whether a viewer on one team is kept from a player on another: a cloaked
/// Wolf is gone for Sheepdogs, and its own team still sees it.
pub fn hidden(viewer: Role, target: Role, cloaked: bool) -> bool {
    cloaked && viewer != target
}

/// How far a viewer sees, through Dognoculars or not. They win back what the
/// night takes and no more: past a day's sight, the list of sheep in sight
/// would come near the count of sheep alive (FL-7).
pub fn scoped_range(range: Fx, scoped: bool) -> Fx {
    if scoped {
        (range * tune::DOGNOCULARS_SIGHT_TIMES).min(tune::DOGNOCULARS_SIGHT_MOST)
    } else {
        range
    }
}

/// Whether this viewer smells scents: a Sheepdog with Sniff Snoff running.
pub fn smells(viewer: Role, sniffing: bool) -> bool {
    viewer == Role::Sheepdog && sniffing
}

/// Everything one-sided on the screen of a viewer on this team.
pub fn view(viewer: Role, facts: &Facts) -> View {
    let is_sheepdog = viewer == Role::Sheepdog;
    View {
        outline_sheepdogs: sees_outline(viewer, Role::Sheepdog),
        outline_wolves: sees_outline(viewer, Role::Wolf),
        eats: facts.standing.eats,
        in_fold: facts.standing.in_fold,
        wolf_killed: is_sheepdog && facts.fallen.iter().any(Fallen::news),
        worn_bells: facts
            .bells
            .iter()
            .filter(|b| is_sheepdog && !b.used_up())
            .map(Bell::sheep)
            .collect(),
        ringing_bells: facts
            .bells
            .iter()
            .filter(|b| b.ringing())
            .map(Bell::sheep)
            .collect(),
        mounds: facts.mounds.iter().map(|m| m.look(viewer)).collect(),
        light: night_light(viewer, facts.darkness),
        sight_range: scoped_range(sight_range(viewer, facts.darkness), facts.scoped),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::tune::Powerup::{Howl, Zoomies};
    use Role::{Sheepdog, Wolf};

    const STANDING: Standing = Standing {
        eats: 7,
        in_fold: 30,
        carcasses_found: 3,
        wolves_alive: 2,
        elapsed: 600,
    };
    const DAY: Facts = Facts {
        standing: &STANDING,
        bells: &[],
        fallen: &[],
        mounds: &[],
        darkness: 0,
        scoped: false,
    };

    #[test]
    fn sheepdogs_hear_of_a_kill_and_wolves_do_not() {
        let fallen = [Fallen::new(0, 0, 0, 1)];
        let facts = Facts {
            fallen: &fallen,
            ..DAY
        };
        assert!(view(Sheepdog, &facts).wolf_killed);
        assert!(!view(Wolf, &facts).wolf_killed);
        assert!(!view(Sheepdog, &DAY).wolf_killed);
    }

    #[test]
    fn sheepdogs_see_sheepdogs_and_no_wolves() {
        let v = view(Sheepdog, &DAY);
        assert!(v.outline_sheepdogs);
        assert!(!v.outline_wolves);
    }

    #[test]
    fn wolves_see_wolves_and_sheepdogs() {
        let v = view(Wolf, &DAY);
        assert!(v.outline_wolves);
        assert!(v.outline_sheepdogs);
    }

    #[test]
    fn rolling_wolves_see_dogs_back_hides_only_sheepdogs_from_wolves() {
        assert!(!outline_rule(Wolf, Sheepdog, false));
        assert!(outline_rule(Wolf, Wolf, false));
        assert!(outline_rule(Sheepdog, Sheepdog, false));
    }

    #[test]
    fn the_eats_reach_both_teams() {
        assert_eq!(view(Sheepdog, &DAY).eats, 7);
        assert_eq!(view(Wolf, &DAY).eats, 7);
    }

    #[test]
    fn the_count_in_the_fold_reaches_both_teams() {
        assert_eq!(view(Sheepdog, &DAY).in_fold, 30);
        assert_eq!(view(Wolf, &DAY).in_fold, 30);
    }

    #[test]
    fn sheepdogs_see_which_sheep_wears_a_bell() {
        let bells = [Bell::hang(4), Bell::hang(9)];
        let facts = Facts { bells: &bells, ..DAY };
        assert_eq!(view(Sheepdog, &facts).worn_bells, [4, 9]);
    }

    #[test]
    fn a_used_up_bell_leaves_the_sheepdogs_view() {
        let mut used_up = Bell::hang(4);
        used_up.tick(Some(0));
        for _ in 0..600 {
            used_up.tick(None);
        }
        let bells = [used_up];
        let facts = Facts { bells: &bells, ..DAY };
        assert!(view(Sheepdog, &facts).worn_bells.is_empty());
    }

    #[test]
    fn a_wolf_gets_no_sign_of_a_silent_bell() {
        let bells = [Bell::hang(4)];
        let facts = Facts { bells: &bells, ..DAY };
        let v = view(Wolf, &facts);
        assert!(v.worn_bells.is_empty());
        assert!(v.ringing_bells.is_empty());
        let json = serde_json::to_string(&v).unwrap();
        assert!(!json.contains("bells"), "{json}");
    }

    #[test]
    fn a_ringing_bell_reaches_both_teams_as_a_sound() {
        let mut ringing = Bell::hang(4);
        ringing.tick(Some(0));
        let bells = [ringing, Bell::hang(9)];
        let facts = Facts { bells: &bells, ..DAY };
        assert_eq!(view(Sheepdog, &facts).ringing_bells, [4]);
        let wolf = view(Wolf, &facts);
        assert_eq!(wolf.ringing_bells, [4]);
        assert!(wolf.worn_bells.is_empty());
    }

    #[test]
    fn a_decoy_reaches_sheepdogs_as_a_plain_mound() {
        let mounds = [Mound::decoy(), Mound::buried(Howl, Zoomies)];
        let facts = Facts {
            mounds: &mounds,
            ..DAY
        };
        assert_eq!(
            view(Sheepdog, &facts).mounds,
            [MoundLook::Fresh, MoundLook::Fresh]
        );
        assert_eq!(view(Wolf, &facts).mounds, [MoundLook::Decoy, MoundLook::Fresh]);
    }

    #[test]
    fn by_day_both_teams_see_the_same_distance() {
        assert_eq!(view(Sheepdog, &DAY).sight_range, fx::from_int(150));
        assert_eq!(view(Wolf, &DAY).sight_range, fx::from_int(150));
    }

    #[test]
    fn by_night_wolves_keep_more_light_than_sheepdogs() {
        assert_eq!(night_light(Sheepdog, fx::ONE), fx::ratio(50, 100));
        assert_eq!(night_light(Wolf, fx::ONE), fx::ratio(80, 100));
    }

    #[test]
    fn night_comes_on_evenly_and_leaves_sight_alone() {
        assert_eq!(night_light(Sheepdog, 0), fx::ONE);
        // halfway from 100% to 50% is 75%
        assert!((night_light(Sheepdog, fx::HALF) - fx::ratio(75, 100)).abs() < 2);
        assert_eq!(sight_range(Sheepdog, fx::ONE), fx::from_int(150));
    }
}
