//! The powerup slot: hold one, activate it, discard it. `tune.rs` holds the
//! table; every rule here reads a row and none names a powerup, so a new row
//! needs nothing added here.
//!
//! A player holds one powerup at a time. A one-use powerup is gone on
//! activation. A timed one leaves the slot on activation and runs on beside
//! it, so the player can dig again, and two different timers can overlap;
//! using the same one twice restarts it. A meter stays in the slot until it's
//! spent or discarded, and drains only while held out.
//!
//! `World` calls `activate` on a right-click press and `release` when the
//! button comes up, and applies what `activate` returns: a Howl starts the
//! `Night`, a Bell needs a sheep under the crosshair first, so `World`
//! activates it only when there is one, and a Bone goes off at the end of a
//! hold, when its decoy `Mound` is planted.

use super::tune::{self, Powerup, PowerupRow, Usage, POWERUPS};
use super::Role;
use crate::fx::{self, Fx};
use crate::rng::Rng;

/// This powerup's row of the table.
pub fn row(kind: Powerup) -> &'static PowerupRow {
    &POWERUPS[kind as usize]
}

/// One team's powerups, in table order.
pub fn rows_for(role: Role) -> impl Iterator<Item = &'static PowerupRow> {
    POWERUPS.iter().filter(move |r| r.role == role)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Held {
    kind: Powerup,
    /// Ticks of use left on a meter.
    meter: i32,
    /// Whether a meter is held out now.
    out: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PowerupSlot {
    held: Option<Held>,
    /// Ticks left on each timed powerup, by row of the table.
    running: [i32; POWERUPS.len()],
}

impl PowerupSlot {
    pub fn held(&self) -> Option<Powerup> {
        self.held.map(|h| h.kind)
    }

    /// Whether this player can dig.
    pub fn is_free(&self) -> bool {
        self.held.is_none()
    }

    /// Fills the slot. False, with nothing changed, when it's already full.
    pub fn take(&mut self, kind: Powerup) -> bool {
        if self.held.is_some() {
            return false;
        }
        let meter = match row(kind).usage {
            Usage::Meter(ticks) => ticks,
            Usage::Once | Usage::Timer(_) => 0,
        };
        self.held = Some(Held {
            kind,
            meter,
            out: false,
        });
        true
    }

    /// A right-click press: puts what's held to use, and says what that was.
    pub fn activate(&mut self) -> Option<Powerup> {
        let held = self.held.as_mut()?;
        if held.out {
            return None;
        }
        let kind = held.kind;
        match row(kind).usage {
            Usage::Once => self.held = None,
            Usage::Timer(ticks) => {
                self.running[kind as usize] = ticks;
                self.held = None;
            }
            Usage::Meter(_) => held.out = true,
        }
        Some(kind)
    }

    /// The right button came up: a meter goes away.
    pub fn release(&mut self) {
        if let Some(held) = self.held.as_mut() {
            held.out = false;
        }
    }

    /// Throws away what's held. A timer already running carries on: it was
    /// used, and isn't held any more. False when the slot was empty.
    pub fn discard(&mut self) -> bool {
        self.held.take().is_some()
    }

    /// The Wolf ate: a running powerup that breaks on eat ends at once.
    pub fn ate(&mut self) {
        for row in POWERUPS.iter().filter(|r| r.breaks_on_eat) {
            self.running[row.kind as usize] = 0;
        }
    }

    /// One tick: timers run down, and a meter held out drains.
    pub fn tick(&mut self) {
        for left in &mut self.running {
            *left = (*left - 1).max(0);
        }
        if let Some(held) = self.held.as_mut() {
            if held.out {
                held.meter -= 1;
                if held.meter == 0 {
                    self.held = None;
                }
            }
        }
    }

    /// Whether this powerup's effect is on now: its timer is running, or its
    /// meter is held out.
    pub fn running(&self, kind: Powerup) -> bool {
        self.running[kind as usize] > 0 || self.held.is_some_and(|h| h.kind == kind && h.out)
    }

    /// The HUD's bar for what's held: what's left of a meter, 1 for anything
    /// else held, 0 for an empty slot.
    pub fn held_fraction(&self) -> Fx {
        match self.held {
            Some(held) => match row(held.kind).usage {
                Usage::Meter(ticks) => fx::ratio(held.meter, ticks),
                Usage::Once | Usage::Timer(_) => fx::ONE,
            },
            None => 0,
        }
    }

    /// The timers running now, in table order.
    pub fn timers(&self) -> impl Iterator<Item = &'static PowerupRow> + '_ {
        POWERUPS.iter().filter(|r| self.running[r.kind as usize] > 0)
    }

    /// The HUD's bar for a running timer: what's left of it.
    pub fn running_fraction(&self, kind: Powerup) -> Fx {
        match row(kind).usage {
            Usage::Timer(ticks) => fx::ratio(self.running[kind as usize], ticks),
            Usage::Once | Usage::Meter(_) => 0,
        }
    }
}

/// How high a Super jump goes, for a sheep this tall.
pub fn super_jump_height(sheep_height: Fx) -> Fx {
    fx::mul(fx::from_int(tune::SUPER_JUMP_SHEEP_HEIGHTS), sheep_height)
}

/// The upward speed that takes a body to a Super jump's height: v^2 = 2 g h.
pub fn super_jump_speed() -> Fx {
    let height = super_jump_height(tune::SUPER_JUMP_SHEEP_HEIGHT);
    fx::sqrt(fx::mul(crate::tune::GRAVITY * 2, height))
}

/// A sprint speed under Zoomies.
pub fn zoomies_sprint(sprint: Fx) -> Fx {
    fx::mul(sprint, fx::ratio(tune::ZOOMIES_SPRINT_PERCENT, 100))
}

/// Where a Wolf's scent shows under Sniff Snoff, as an offset from the Wolf:
/// somewhere near it, in a direction drawn from `rng`, and never on it.
pub fn scent_offset(rng: &mut Rng) -> (Fx, Fx) {
    let angle = rng.below(fx::TURN as u32) as i32;
    let span = (tune::SCENT_DRIFT_MAX - tune::SCENT_DRIFT_MIN) as u32;
    let distance = tune::SCENT_DRIFT_MIN + rng.below(span) as Fx;
    (
        fx::mul(distance, fx::cos(angle)),
        fx::mul(distance, fx::sin(angle)),
    )
}

/// The Dognoculars' zoom after this many wheel notches, up positive.
pub fn dognoculars_zoom(zoom: i32, notches: i32) -> i32 {
    let zoom = zoom + notches * tune::DOGNOCULARS_ZOOM_STEP;
    zoom.clamp(tune::DOGNOCULARS_ZOOM_MIN, tune::DOGNOCULARS_ZOOM_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::tune::Powerup::*;

    fn holding(kind: Powerup) -> PowerupSlot {
        let mut slot = PowerupSlot::default();
        slot.take(kind);
        slot
    }

    fn ticked(slot: &mut PowerupSlot, ticks: i32) {
        for _ in 0..ticks {
            slot.tick();
        }
    }

    #[test]
    fn each_row_holds_what_the_person_settled() {
        use Role::{Sheepdog, Wolf};
        use Usage::{Meter, Once, Timer};
        // Hand-typed from BRIEF.md, with seconds worked into ticks at 60 a
        // second.
        let rows = [
            (
                SuperAppetite,
                Wolf,
                "super-appetite",
                "Super appetite",
                Timer(1800),
                false,
            ),
            (
                InvisibilityCloak,
                Wolf,
                "invisibility-cloak",
                "Invisibility cloak",
                Timer(300),
                true,
            ),
            (SheepSuit, Wolf, "sheep-suit", "Sheep suit", Timer(1800), true),
            (Bone, Wolf, "bone", "Bone", Once, false),
            (Howl, Wolf, "three-wolf-moon", "Howl", Once, false),
            (Stampede, Wolf, "stampede", "Stampede", Once, false),
            (
                CarrotOnAStick,
                Sheepdog,
                "carrot-on-a-stick",
                "Carrot on a stick",
                Meter(600),
                false,
            ),
            (SuperJump, Sheepdog, "super-jump", "Super jump", Once, false),
            (Zoomies, Sheepdog, "zoomies", "Zoomies", Timer(600), false),
            (
                Dognoculars,
                Sheepdog,
                "dognoculars",
                "Dognoculars",
                Meter(600),
                false,
            ),
            (
                SniffSnoff,
                Sheepdog,
                "sniff-snoff",
                "Sniff Snoff",
                Timer(900),
                false,
            ),
            (Bell, Sheepdog, "bell", "Bell", Once, false),
        ];
        for (kind, role, icon, name, usage, breaks_on_eat) in rows {
            let expected = PowerupRow {
                kind,
                role,
                icon,
                name,
                usage,
                breaks_on_eat,
            };
            assert_eq!(*row(kind), expected);
        }
    }

    #[test]
    fn no_two_rows_share_an_icon_id_or_a_display_name() {
        for (i, a) in POWERUPS.iter().enumerate() {
            for b in &POWERUPS[i + 1..] {
                assert_ne!(a.icon, b.icon);
                assert_ne!(a.name, b.name);
            }
        }
    }

    #[test]
    fn a_team_gets_its_own_powerups() {
        assert!(rows_for(Role::Wolf).all(|r| r.role == Role::Wolf));
        assert!(rows_for(Role::Sheepdog).all(|r| r.role == Role::Sheepdog));
        assert_eq!(rows_for(Role::Wolf).next().map(|r| r.kind), Some(SuperAppetite));
        assert_eq!(
            rows_for(Role::Sheepdog).next().map(|r| r.kind),
            Some(CarrotOnAStick)
        );
    }

    #[test]
    fn a_slot_holds_one_powerup() {
        let mut slot = holding(Zoomies);
        assert!(!slot.take(Bell));
        assert_eq!(slot.held(), Some(Zoomies));
    }

    #[test]
    fn activating_an_empty_slot_does_nothing() {
        assert_eq!(PowerupSlot::default().activate(), None);
    }

    #[test]
    fn a_held_powerup_does_nothing_until_it_is_activated() {
        let mut slot = holding(Zoomies);
        ticked(&mut slot, 1000);
        assert_eq!(slot.held(), Some(Zoomies));
        assert!(!slot.running(Zoomies));
    }

    #[test]
    fn a_timed_powerup_frees_the_slot_and_runs_its_ticks() {
        let timed = [
            (SuperAppetite, 1800),
            (InvisibilityCloak, 300),
            (SheepSuit, 1800),
            (Zoomies, 600),
            (SniffSnoff, 900),
        ];
        for (kind, ticks) in timed {
            let mut slot = holding(kind);
            assert_eq!(slot.activate(), Some(kind));
            assert!(slot.is_free(), "{kind:?}");
            ticked(&mut slot, ticks - 1);
            assert!(slot.running(kind), "{kind:?}");
            slot.tick();
            assert!(!slot.running(kind), "{kind:?}");
        }
    }

    #[test]
    fn a_one_use_powerup_is_spent_on_activation() {
        for kind in [Bone, Howl, Stampede, SuperJump, Bell] {
            let mut slot = holding(kind);
            assert_eq!(slot.activate(), Some(kind));
            assert!(slot.is_free(), "{kind:?}");
            assert!(!slot.running(kind), "{kind:?}");
        }
    }

    #[test]
    fn a_meter_drains_only_while_it_is_held_out() {
        let mut slot = holding(CarrotOnAStick);
        ticked(&mut slot, 100);
        assert_eq!(slot.held_fraction(), fx::ONE);
        assert_eq!(slot.activate(), Some(CarrotOnAStick));
        assert!(slot.running(CarrotOnAStick));
        ticked(&mut slot, 150);
        slot.release();
        assert!(!slot.running(CarrotOnAStick));
        ticked(&mut slot, 50);
        // 150 of 600 ticks used leaves three quarters: 49152 / 65536
        assert_eq!(slot.held_fraction(), 49152);
        assert_eq!(slot.held(), Some(CarrotOnAStick));
    }

    #[test]
    fn a_meter_held_out_again_keeps_draining_from_where_it_stopped() {
        let mut slot = holding(Dognoculars);
        slot.activate();
        ticked(&mut slot, 300);
        slot.release();
        slot.activate();
        ticked(&mut slot, 299);
        assert_eq!(slot.held(), Some(Dognoculars));
        slot.tick();
        assert_eq!(slot.held(), None);
    }

    #[test]
    fn a_meter_that_runs_out_empties_the_slot() {
        for kind in [CarrotOnAStick, Dognoculars] {
            let mut slot = holding(kind);
            slot.activate();
            ticked(&mut slot, 599);
            assert!(slot.running(kind), "{kind:?}");
            slot.tick();
            assert!(slot.is_free(), "{kind:?}");
            assert!(!slot.running(kind), "{kind:?}");
        }
    }

    #[test]
    fn a_meter_already_out_does_not_activate_again() {
        let mut slot = holding(Dognoculars);
        slot.activate();
        assert_eq!(slot.activate(), None);
    }

    #[test]
    fn two_different_timers_can_overlap() {
        let mut slot = holding(SheepSuit);
        slot.activate();
        ticked(&mut slot, 100);
        slot.take(InvisibilityCloak);
        slot.activate();
        assert!(slot.running(SheepSuit));
        assert!(slot.running(InvisibilityCloak));
        // the cloak's 300 ticks end first
        ticked(&mut slot, 300);
        assert!(slot.running(SheepSuit));
        assert!(!slot.running(InvisibilityCloak));
    }

    #[test]
    fn using_the_same_timer_again_restarts_it() {
        let mut slot = holding(Zoomies);
        slot.activate();
        ticked(&mut slot, 500);
        slot.take(Zoomies);
        slot.activate();
        ticked(&mut slot, 599);
        assert!(slot.running(Zoomies));
        slot.tick();
        assert!(!slot.running(Zoomies));
    }

    #[test]
    fn discarding_empties_the_slot_and_says_whether_it_held_anything() {
        let mut slot = holding(Bone);
        assert!(slot.discard());
        assert!(slot.is_free());
        assert!(!slot.discard());
    }

    #[test]
    fn discarding_leaves_a_running_timer_alone() {
        let mut slot = holding(Zoomies);
        slot.activate();
        slot.take(Bell);
        slot.discard();
        assert!(slot.running(Zoomies));
    }

    #[test]
    fn eating_breaks_a_running_cloak_or_suit_at_once() {
        for kind in [InvisibilityCloak, SheepSuit] {
            let mut slot = holding(kind);
            slot.activate();
            slot.ate();
            assert!(!slot.running(kind), "{kind:?}");
        }
    }

    #[test]
    fn eating_leaves_super_appetite_and_a_held_cloak_alone() {
        let mut slot = holding(SuperAppetite);
        slot.activate();
        slot.take(InvisibilityCloak);
        slot.ate();
        assert!(slot.running(SuperAppetite));
        assert_eq!(slot.held(), Some(InvisibilityCloak));
    }

    #[test]
    fn the_hud_reads_what_is_held_and_what_is_running() {
        let mut slot = PowerupSlot::default();
        assert_eq!(slot.held_fraction(), 0);
        slot.take(Bell);
        assert_eq!(slot.held_fraction(), fx::ONE);
        slot.discard();
        slot.take(Zoomies);
        assert_eq!(slot.held_fraction(), fx::ONE);
        slot.activate();
        // 150 of 600 ticks used leaves three quarters: 49152 / 65536
        ticked(&mut slot, 150);
        assert_eq!(slot.running_fraction(Zoomies), 49152);
        assert_eq!(slot.running_fraction(Bell), 0);
    }

    #[test]
    fn a_super_jump_is_five_sheep_high() {
        // A ewe's 0.62 m is 40632 in Q16.16, and five of those is 203160
        assert_eq!(super_jump_height(40632), 203_160);
    }

    #[test]
    fn a_super_jump_leaves_the_ground_at_eleven_meters_a_second() {
        // 3.1 m under 20 m/s^2: the square root of 124 is 11.14
        assert_eq!(fx::round(super_jump_speed() * 10), 111);
    }

    #[test]
    fn zoomies_sprint_at_three_quarters_over() {
        // 175% of 16 m/s is 28 m/s
        assert_eq!(zoomies_sprint(fx::from_int(16)), fx::from_int(28));
    }

    #[test]
    fn a_scent_offset_comes_from_the_seed() {
        // Worked out by hand: seed 0's first value gives 57888 of 65536 turns,
        // or 318 degrees, and its second 3 + 7 * 0.4315 = 6.02 m. So x is about
        // 6.02 * 0.743 = 4.47 and z is about 6.02 * -0.669 = -4.03.
        let (x, z) = scent_offset(&mut Rng::new(0));
        assert_eq!((fx::round(x), fx::round(z)), (4, -4));
    }

    #[test]
    fn a_wolfs_scent_is_never_on_it_and_stays_within_about_ten_meters() {
        let mut rng = Rng::new(7);
        for _ in 0..1000 {
            let (x, z) = scent_offset(&mut rng);
            let distance = fx::length(x, z);
            // fx::sin and fx::cos are within 0.002, so the length is within 0.3%
            assert!(distance > fx::ratio(29, 10), "{distance}");
            assert!(distance < fx::ratio(101, 10), "{distance}");
        }
    }

    #[test]
    fn the_dognoculars_zoom_stays_from_three_to_ten() {
        assert_eq!(dognoculars_zoom(3, 1), 4);
        assert_eq!(dognoculars_zoom(9, 3), 10);
        assert_eq!(dognoculars_zoom(4, -3), 3);
    }
}
