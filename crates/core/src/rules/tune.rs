//! Every balance number the match rules use, in one table. The person's
//! numbers were spitballed and may move (FL-2), so each lives here once and
//! the rules read it from here. The comment on each line says whose it is:
//!
//! - `person`: the person's, from `BRIEF.md` or `DECISIONS.md`.
//! - `contract`: from `notes/contract.md`.
//! - `agent`: a limit the brief left open (`SECOND-ORDER.md`, R1.609).
//! - `stub`: the brief names the idea and gives no number, so this stands in
//!   for one until the person or the balance runs pick it (R1.609).
//!
//! The powerup table at the foot is where a powerup is declared. One row
//! gives it a kind, a team, an icon id, a display name and its timer or
//! meter, and no other file lists powerups, so a new one is one new row.

use serde::Serialize;

use super::Role;
use crate::fx::{from_int, Fx};
use crate::ticks::TICKS_PER_SECOND;

/// Seconds as ticks.
pub const fn secs(seconds: i32) -> i32 {
    seconds * TICKS_PER_SECOND as i32
}

// ---- Seats and settings ----

pub const MIN_SEATS: usize = 2; // person: one on each team
pub const MAX_SEATS: usize = 8; // person
pub const MATCH_MINUTES: i32 = 8; // person
pub const MATCH_MINUTES_MIN: i32 = 1; // agent
pub const MATCH_MINUTES_MAX: i32 = 30; // agent
pub const SHEEP_FIRST_WOLF: i32 = 100; // person: the base for 2 Wolves and up
pub const SHEEP_LONE_WOLF: i32 = 80; // bot table, person agreed: a 1 v 1 ran close
pub const SHEEP_PER_EXTRA_WOLF: i32 = 50; // person
pub const SHEEP_MIN: i32 = 10; // agent
pub const SHEEP_MAX: i32 = 400; // contract: seven Wolves' default
pub const EATS_PER_WOLF: i32 = 8; // bot table, person agreed
pub const EATS_LONE_WOLF: i32 = 9; // bot table, person agreed
pub const EATS_MIN: i32 = 1; // agent
/// With Defaults on, the most players a Small map takes, then a Medium one;
/// more players get Big.
pub const MAP_SMALL_MAX_PLAYERS: i32 = 3; // agent
pub const MAP_MEDIUM_MAX_PLAYERS: i32 = 5; // agent
pub const POWERUPS_ON: bool = true; // person
pub const SUDDEN_DEATH: bool = false; // person

// ---- The Fold ----

/// Sheep to reach the Fold, per Sheepdog. Only the Sheepdogs change it.
pub const FOLD_PER_SHEEPDOG: i32 = 60; // person: scale with Sheepdogs; number from the bot table

// ---- Wolves ----

pub const EAT_COOLDOWN_SECONDS: i32 = 20; // person: flat, whatever the wolf count
pub const EAT_LOCK_SECONDS: i32 = 1; // person
pub const SUPER_APPETITE_SPEED: i32 = 2; // person: doubles the dial's speed
pub const WOLF_RETURN_SECONDS: i32 = 10; // person: "lower the cooldown on death to 10 seconds"
pub const TUNNEL_COOLDOWN_SECONDS: i32 = 30; // person

// ---- Sheepdogs ----

pub const LUNGE_REACH: Fx = from_int(54) / 5; // person: "like 4x as far" (T17), 10.8 m so the body still goes 9.2 m
pub const LUNGE_TICKS: i32 = 24; // agent: 0.4 s, half as fast again as a sprint (R1.2300)
pub const LUNGE_MISS_SECONDS: i32 = 4; // person: 6 less a third
pub const BARK_COOLDOWN_SECONDS: i32 = 3; // person
pub const KILL_BODY_TICKS: i32 = secs(4); // coordinator: how long a killed Wolf lies
pub const KILL_NEWS_TICKS: i32 = secs(2); // coordinator: how long the Sheepdogs' screens show it
pub const KILL_FLASH_TICKS: i32 = 15; // agent: the killer's flash, a quarter second

/// How far a Sheepdog's body goes in a Lunge, short of its reach by this
/// much: a Wolf this close at any moment of the Lunge is hit.
pub const LUNGE_CONTACT: Fx = from_int(8) / 5; // person: twice as generous

/// How far a Lunge carries a Sheepdog in a tick. Less than `LUNGE_CONTACT`, so
/// it passes no Wolf, and less than a wall is thick, so it passes no wall.
pub const LUNGE_STRIDE: Fx = (LUNGE_REACH - LUNGE_CONTACT) / LUNGE_TICKS;

// ---- The crosshair, until the camera's picking lands (PLAN 12) ----

pub const AIM_WIDTH: Fx = from_int(4) / 5; // stub: how far off the line still counts
pub const EAT_REACH: Fx = from_int(3); // agent: no steep look down to eat (R1.1602)
pub const DIG_REACH: Fx = from_int(3); // stub
pub const TUNNEL_REACH: Fx = from_int(3); // stub
pub const CARCASS_SIGHT_REACH: Fx = from_int(30); // stub
pub const SIGHT_STEP: Fx = from_int(1); // stub: how often a line of sight is tested

// ---- Wolves returning ----

pub const RETURN_CLEAR: Fx = from_int(40); // stub: "a random hole away from dogs"

// ---- Mounds ----

pub const MOUNDS_PER_PLAYER: i32 = 2; // person: scale with players
/// A new mound comes up this many seconds apart, shared among the players:
/// one a minute with four.
pub const NEW_MOUND_PLAYER_SECONDS: i32 = 240; // person: scale with players
pub const DIG_SECONDS: i32 = 2; // person

// ---- Powerup numbers that are more than a timer or a meter ----

pub const BONE_STUN_SECONDS: i32 = 10; // person
pub const BONE_PLANT_SECONDS: i32 = 3; // person: T14
pub const BONE_PLANT_REACH: Fx = from_int(5); // stub: how near a tree or cairn
pub const BONE_BASE_GAP: Fx = from_int(1) / 2; // stub: the decoy, out from the base
pub const BELL_HANG_REACH: Fx = from_int(8); // stub
pub const SCENT_REDRAW_SECONDS: i32 = 3; // coordinator
pub const HOWL_NIGHT_SECONDS: i32 = 60; // person
pub const HOWL_CHANGE_SECONDS: i32 = 3; // person
pub const STAMPEDE_REACH: Fx = from_int(15); // person: about 15 m
pub const BELL_REACH: Fx = from_int(20); // person: "way farther, like 20m at least"
pub const BELL_RING_SECONDS: i32 = 10; // person: about 10 s
pub const DECOY_STUN_SECONDS: i32 = 5; // agent: the coordinator's number for a Wolf that bites a Decoy sheep
pub const DECOY_AHEAD: Fx = from_int(2); // agent: how far ahead of the Sheepdog a Decoy sheep stands
pub const FRENZY_CONTACT: Fx = from_int(3) / 2; // agent: a Wolf in a Frenzy kills a Sheepdog this near, body to body
pub const DOGNOCULARS_ZOOM_MIN: i32 = 3; // person
pub const DOGNOCULARS_ZOOM_MAX: i32 = 10; // person
pub const DOGNOCULARS_ZOOM_STEP: i32 = 1; // stub: one wheel notch
pub const DOGNOCULARS_SIGHT_TIMES: i32 = 3; // stub: how much farther the spyglass sees
pub const DOGNOCULARS_SIGHT_MOST: Fx = SIGHT_SHEEPDOG_DAY; // agent: keeps most of the flock off the list (FL-7)
pub const DOGNOCULARS_CARCASS_REACH: Fx = from_int(90); // stub: a carcass found through it
pub const ZOOMIES_SPRINT_PERCENT: i32 = 175; // stub: "faster sprint speed"
pub const SCENT_DRIFT_MIN: Fx = from_int(3); // stub: a Wolf's scent is "never on it"
pub const SCENT_DRIFT_MAX: Fx = from_int(10); // person: about 10 m from the Wolf

// ---- Sight, in meters ----

pub const SIGHT_SHEEPDOG_DAY: Fx = from_int(150); // stub
pub const SIGHT_SHEEPDOG_NIGHT: Fx = SIGHT_SHEEPDOG_DAY; // person: night is dark, not a short draw distance
pub const SIGHT_WOLF_DAY: Fx = from_int(150); // stub
pub const SIGHT_WOLF_NIGHT: Fx = SIGHT_WOLF_DAY; // person: as above
/// How much of the light a team has left at full night, in percent. Wolves
/// keep more, which is how they see "clearly further" by night.
pub const NIGHT_LIGHT_SHEEPDOG: i32 = 50; // person: "much darker"
pub const NIGHT_LIGHT_WOLF: i32 = 80; // agent

// ---- Bots ----

/// How often a bot decides, and how far apart two seats' turns fall, so no
/// two bots decide on the same tick. Between decisions it only steers.
pub const BOT_THINK_TICKS: u32 = 24; // agent
pub const BOT_THINK_STAGGER: u32 = 3; // agent
pub const BOT_TURN: i32 = 1400; // agent: most a bot turns in a tick, about 460 degrees a second
pub const BOT_AIMED: i32 = 1200; // agent: this near its target's yaw, a bot presses
pub const BOT_ARRIVED: Fx = from_int(3) / 5; // agent
pub const BOT_DOOR_PAST: Fx = from_int(3); // agent: how far either side of a gap to aim
pub const BOT_DOOR_LINE: Fx = from_int(3) / 2; // agent: this near the gap's line, go through
pub const BOT_STUCK_MOVE: Fx = from_int(1) / 5; // agent: less than this in a decision is stuck
pub const BOT_STUCK_THINKS: u8 = 3; // agent
pub const BOT_DODGE_TICKS: i32 = 40; // agent
pub const BOT_WALL_ROOM: Fx = from_int(1); // agent: kept from a wall when picking a spot
pub const BOT_RUN_FROM: Fx = from_int(12); // agent: a Sheepdog sprints to a post this far off

// What a Sheepdog bot can tell, and how far off.
pub const BOT_TELL_EATING: Fx = from_int(40); // agent
pub const BOT_TELL_RUNNING: Fx = from_int(60); // agent
pub const BOT_TELL_TALL: Fx = from_int(25); // agent
pub const BOT_TELL_BARK: Fx = from_int(12); // agent: what stands still through a bark
pub const BOT_BARK_LOOK_TICKS: u32 = 48; // agent: how long after a bark it looks for that
pub const BOT_QUARRY_TICKS: i32 = secs(6); // agent: how long a bot keeps its eye on a Wolf
pub const BOT_LUNGE_FROM: Fx = from_int(6); // agent: within `LUNGE_REACH`, less what a Wolf walks in a Lunge
pub const BOT_CARCASS_REACH: Fx = from_int(25); // agent: within `CARCASS_SIGHT_REACH`

// Either team's mounds and powerups.
pub const BOT_MOUND_REACH: Fx = from_int(20); // agent: how far out of its way for a mound
pub const BOT_DIG_FROM: Fx = from_int(5) / 2; // agent: within `DIG_REACH` and `TUNNEL_REACH`
pub const BOT_HOLD_TICKS: i32 = secs(10); // agent: then a powerup that won't go off is thrown away

// How a Sheepdog bot herds, after the scripted Sheepdog of `tests/common`.
pub const BOT_PRESSURE: Fx = from_int(9) / 2; // agent: how far behind a sheep to stand
pub const BOT_FLANK: Fx = from_int(10); // agent: how wide to pass it
pub const BOT_STAND_OFF: Fx = from_int(7) / 2; // agent: stop short of a sheep this near
pub const BOT_BARK_REACH: Fx = from_int(4); // agent: past where it stands
pub const BOT_MOVING: Fx = from_int(1); // agent: a sheep slower than this needs more pressure
pub const BOT_BARK_TICKS: u32 = 300; // agent
pub const BOT_JOB_TICKS: i32 = secs(45); // agent: the longest spent driving one sheep to its gap
pub const BOT_LEAVE_BEHIND: i32 = 3; // agent: a field with this few left isn't worth staying for

// How a Wolf bot stays hidden.
pub const BOT_FLEE_REACH: Fx = from_int(4); // agent: a Sheepdog this near sends it running; more and none is ever caught (R1.1806)
pub const BOT_FLEE_RUN: i32 = 20; // agent: meters a flight aims ahead
pub const BOT_FRENZY_REACH: Fx = from_int(15); // coordinator: a Wolf bot holding Frenzy uses it on a Sheepdog this near
pub const BOT_FRENZY_CHASE: Fx = from_int(40); // agent: a Wolf bot in a Frenzy runs at a Sheepdog this near
pub const BOT_FRENZY_FLEE: Fx = from_int(30); // agent: a Sheepdog bot runs from a Wolf in a Frenzy this near
pub const BOT_HIDE_REACH: Fx = from_int(30); // agent: crouched with a Sheepdog this near
pub const BOT_EAT_SAFE: Fx = from_int(25); // agent: no meal with a Sheepdog this near
pub const BOT_EAT_FROM: Fx = from_int(5) / 2; // agent: within `EAT_REACH`
pub const BOT_MINGLE: Fx = from_int(2); // agent: how near a sheep to stay
pub const BOT_MOVE_ON: Fx = from_int(15); // agent: how far from a meal to go next
pub const BOT_MOUTH_REACH: Fx = from_int(30); // agent: how far it runs for a tunnel
pub const BOT_MEAL_EVERY_TICKS: u32 = secs(44) as u32; // agent: a person's pace, not the dial's

// ---- Information ----

/// Whether Wolves see the outlines of Sheepdogs. The person wasn't sure:
/// "make it easy to roll this back". This line is the roll-back (FL-7).
pub const WOLVES_SEE_DOGS: bool = true; // person

// ---- Powerups ----

/// How a powerup is spent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Usage {
    /// Gone the moment it's activated.
    Once,
    /// Runs for this many ticks from activation, and frees the slot at once.
    Timer(i32),
    /// Stays in the slot, and drains by this many ticks in all, only while
    /// it's held out.
    Meter(i32),
}

/// One row of the powerup table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PowerupRow {
    pub kind: Powerup,
    pub role: Role,
    pub icon: &'static str,
    pub name: &'static str,
    pub usage: Usage,
    /// Whether eating ends it while it's running.
    pub breaks_on_eat: bool,
}

/// Makes the `Powerup` enum and the table from one list of rows, so a row is
/// all a new powerup needs.
macro_rules! powerup_table {
    ($($kind:ident, $role:ident, $icon:literal, $name:literal, $usage:expr, $breaks:literal;)*) => {
        /// A powerup. A row of the table below; `kind as usize` is its row.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
        pub enum Powerup {
            $($kind),*
        }

        /// Every powerup, in the order of `Powerup`.
        pub const POWERUPS: &[PowerupRow] = &[$(PowerupRow {
            kind: Powerup::$kind,
            role: Role::$role,
            icon: $icon,
            name: $name,
            usage: $usage,
            breaks_on_eat: $breaks,
        }),*];
    };
}

powerup_table! {
    // kind            team      icon id               display name          spent as                breaks on eat
    SuperAppetite,     Wolf,     "super-appetite",     "Super appetite",     Usage::Timer(secs(30)), false;
    InvisibilityCloak, Wolf,     "invisibility-cloak", "Invisibility cloak", Usage::Timer(secs(5)),  true;
    SheepSuit,         Wolf,     "sheep-suit",         "Sheep suit",         Usage::Timer(secs(30)), true;
    Bone,              Wolf,     "bone",               "Bone",               Usage::Once,            false;
    Howl,              Wolf,     "three-wolf-moon",    "Howl",               Usage::Once,            false;
    Stampede,          Wolf,     "stampede",           "Stampede",           Usage::Once,            false;
    Frenzy,            Wolf,     "frenzy",             "Frenzy",             Usage::Timer(secs(8)),  false;
    CarrotOnAStick,    Sheepdog, "carrot-on-a-stick",  "Carrot on a stick",  Usage::Meter(secs(10)), false;
    DecoySheep,        Sheepdog, "decoy-sheep",        "Decoy sheep",        Usage::Once,            false;
    Zoomies,           Sheepdog, "zoomies",            "Zoomies",            Usage::Timer(secs(10)), false;
    Dognoculars,       Sheepdog, "dognoculars",        "Dognoculars",        Usage::Meter(secs(10)), false;
    SniffSnoff,        Sheepdog, "sniff-snoff",        "Sniff Snoff",        Usage::Timer(secs(15)), false;
    Bell,              Sheepdog, "bell",               "Bell",               Usage::Once,            false;
}
