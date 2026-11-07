//! The two win tunes, written out for the Great Highland Bagpipe.
//!
//! Both melodies are traditional. `LICENSES.md` names where they were read
//! from. A piper's chanter has nine notes, no rests and no tonguing, so a tune
//! here is nine pitches and the gracenotes that part them. Nothing in this
//! file sounds: `sound.rs` turns the steps below into a schedule for the page,
//! using the tuning and the timbre in `data/sound.json`.
//!
//! The march's bars hold the plain melody and no gracenote. Every gracenote it
//! plays comes from `grace_march`, a rule written here from the customs of
//! piping, not from any score. The lament's gracenotes are this project's own
//! and are written into its bars.
//!
//! # Reading a bar
//!
//! A bar is a line of words, close to how a piper's score is typed:
//!
//! - A note is its name and length: `la4` is a low A quarter. The names are
//!   `lg la b c d e f hg ha`. Lengths are `4 8 16 32`, a dot adds half
//!   (`la4.`), two dots add three quarters (`b8..`), and `16t` is one of three
//!   in an eighth.
//! - A word before a note decorates it: `gg` a high G gracenote, `tg` a high A
//!   gracenote, `eg` an E gracenote, `dbl` a doubling, `grp` a grip, `thr` a
//!   throw on D.
//! - `~108` sets the pace from there on, as a percent of the beat's length.
//!   A tune played freely is written with these.

/// Parts of a beat a tune's lengths count in, so that a 32nd (3) and a third
/// of an eighth (4) are both whole.
pub const UNITS_PER_BEAT: i64 = 24;

/// The chanter's nine notes, lowest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Pitch {
    LowG,
    LowA,
    B,
    C,
    D,
    E,
    F,
    HighG,
    HighA,
}

impl Pitch {
    pub const ALL: [Pitch; 9] = [
        Pitch::LowG,
        Pitch::LowA,
        Pitch::B,
        Pitch::C,
        Pitch::D,
        Pitch::E,
        Pitch::F,
        Pitch::HighG,
        Pitch::HighA,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Pitch::LowG => "lg",
            Pitch::LowA => "la",
            Pitch::B => "b",
            Pitch::C => "c",
            Pitch::D => "d",
            Pitch::E => "e",
            Pitch::F => "f",
            Pitch::HighG => "hg",
            Pitch::HighA => "ha",
        }
    }

    pub fn from_key(key: &str) -> Option<Pitch> {
        Pitch::ALL.into_iter().find(|p| p.key() == key)
    }
}

/// One melody note: its pitch, its length, where it falls, the gracenotes
/// played into it, and the pace in force.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub pitch: Pitch,
    pub units: i64,
    /// units from the start of its bar
    pub at: i64,
    pub graces: Vec<Pitch>,
    pub pace: i64,
}

/// A strain: the bars between two repeat marks in a piper's score.
#[derive(Debug, Clone, Copy)]
pub struct Strain {
    pub id: &'static str,
    pub bars: &'static [&'static str],
}

/// A whole tune.
#[derive(Debug, Clone, Copy)]
pub struct Score {
    pub id: &'static str,
    /// beats in a bar
    pub meter: i64,
    /// the notes before the first full bar
    pub lift: &'static str,
    /// the bars are the plain melody and `grace_march` adds every gracenote
    pub graced_by_rule: bool,
    pub strains: &'static [Strain],
}

/// What the chanter plays from `at_ms` until the next step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    pub at_ms: i64,
    pub pitch: Pitch,
    /// a gracenote, not a melody note
    pub grace: bool,
}

// ---- the tunes ----

const STB_OPEN: &str = "la4 la8. b16 c8 la8 c8 e8";
const STB_HIGH: &str = "ha4 ha4 ha8 e8 c8 la8";
const STB_TURN: &str = "d4 f8. d16 c8 e8 c8 la8";

/// Scotland the Brave, a 4/4 march: the first part only, the eight bars most
/// people would hum, closing on low A. These bars are the plain melody:
/// `grace_march` adds every gracenote.
pub const SCOTLAND_THE_BRAVE: Score = Score {
    id: "scotland_the_brave",
    meter: 4,
    lift: "e8",
    graced_by_rule: true,
    strains: &[
        Strain {
            id: "opening",
            bars: &[STB_OPEN, STB_HIGH, STB_TURN, "b4 e4 e8. f16 e16. d32 c16. b32"],
        },
        Strain {
            id: "close",
            bars: &[STB_OPEN, STB_HIGH, STB_TURN, "b4 la8. b16 la4."],
        },
    ],
};

const FOF_RISE: &str = "~102 gg la4. grp la4 gg b8 ~98 eg la4 gg b16. c32 thr d4 ~88 gg e16t f16t hg16t";
const FOF_FALL: &str = "~106 tg f4. ~100 dbl e4 gg d8 ~98 gg b8 dbl d4 dbl b8.. la32 ~114 gg lg8";

/// The Flowers of the Forest, the slow lament: the opening phrases, in bars of
/// six beats, played freely, closing on D.
pub const FLOWERS_OF_THE_FOREST: Score = Score {
    id: "flowers_of_the_forest",
    meter: 6,
    lift: "e8",
    graced_by_rule: false,
    strains: &[Strain {
        id: "opening",
        bars: &[
            FOF_RISE,
            FOF_FALL,
            FOF_RISE,
            "~106 tg f4. ~104 dbl e4 gg f8 ~112 dbl d4. ~122 gg d4.",
        ],
    }],
};

pub const SCORES: [Score; 2] = [SCOTLAND_THE_BRAVE, FLOWERS_OF_THE_FOREST];

pub fn score(id: &str) -> Option<Score> {
    SCORES.into_iter().find(|s| s.id == id)
}

impl Score {
    pub fn strain(&self, id: &str) -> Option<Strain> {
        self.strains.iter().find(|s| s.id == id).copied()
    }
}

// ---- reading the notation ----

/// The gracenotes of a doubling on this note, as every piping tutor gives
/// them: a G gracenote, the note, then the gracenote above it. High G and high
/// A have no G gracenote to start from, so they begin on the note itself and
/// are cut from below.
fn doubling(main: Pitch) -> Vec<Pitch> {
    use Pitch::*;
    match main {
        LowG | LowA | B | C => vec![HighG, main, D],
        D => vec![HighG, D, E],
        E => vec![HighG, E, F],
        F => vec![HighG, F, HighG],
        HighG => vec![HighG, F],
        HighA => vec![HighA, HighG],
    }
}

/// The notes of a throw on D, played before the D itself.
const THROW: [Pitch; 3] = [Pitch::LowG, Pitch::D, Pitch::C];

/// What a word before a note in a written bar plays, or `None` if the word is
/// no embellishment.
fn embellish(word: &str, main: Pitch) -> Option<Vec<Pitch>> {
    use Pitch::*;
    Some(match word {
        "gg" => vec![HighG],
        "tg" => vec![HighA],
        "eg" => vec![E],
        "dbl" => doubling(main),
        "grp" => vec![LowG, D, LowG],
        "thr" => THROW.to_vec(),
        _ => return None,
    })
}

/// A note word's pitch and length in units.
fn read_note(word: &str) -> Result<(Pitch, i64), String> {
    let split = word.find(|c: char| c.is_ascii_digit()).unwrap_or(word.len());
    let (name, length) = word.split_at(split);
    let pitch = Pitch::from_key(name).ok_or_else(|| format!("\"{word}\" names no chanter note."))?;
    let (digits, marks) = length.split_at(length.find(['.', 't']).unwrap_or(length.len()));
    let whole = match digits {
        "4" => UNITS_PER_BEAT,
        "8" => UNITS_PER_BEAT / 2,
        "16" => UNITS_PER_BEAT / 4,
        "32" => UNITS_PER_BEAT / 8,
        _ => return Err(format!("\"{word}\" has no length of 4, 8, 16 or 32.")),
    };
    let units = match marks {
        "" => whole,
        "." => whole * 3 / 2,
        ".." => whole * 7 / 4,
        "t" => whole * 2 / 3,
        _ => return Err(format!("\"{word}\" has a mark that means nothing.")),
    };
    Ok((pitch, units))
}

/// The notes of one bar. `pace` carries from bar to bar.
pub fn read_bar(text: &str, pace: &mut i64) -> Result<Vec<Note>, String> {
    let mut notes = Vec::new();
    let mut pending: Option<&str> = None;
    let mut at = 0;
    for word in text.split_whitespace() {
        if let Some(percent) = word.strip_prefix('~') {
            *pace = percent.parse().map_err(|_| format!("\"{word}\" is no pace."))?;
            continue;
        }
        let digit = word.contains(|c: char| c.is_ascii_digit());
        if !digit {
            pending = Some(word);
            continue;
        }
        let (pitch, units) = read_note(word)?;
        let graces = match pending.take() {
            Some(w) => embellish(w, pitch).ok_or_else(|| format!("\"{w}\" is no embellishment."))?,
            None => Vec::new(),
        };
        notes.push(Note {
            pitch,
            units,
            at,
            graces,
            pace: *pace,
        });
        at += units;
    }
    match pending {
        Some(w) => Err(format!("\"{w}\" decorates no note.")),
        None => Ok(notes),
    }
}

// ---- gracing a march by rule ----
//
// Every gracenote of a march comes from here. This is what any piping tutor
// teaches for a march, written as a rule and copied from no score: it sees only
// a note's pitch and length, where it falls in the bar and the note before it,
// and it names no tune and no bar.

/// The single gracenote that parts a note from its twin or marks a beat: high
/// G on every note but high G itself, which takes the thumb's high A.
fn cut(pitch: Pitch) -> Pitch {
    if pitch == Pitch::HighG {
        Pitch::HighA
    } else {
        Pitch::HighG
    }
}

/// The gracenote on a note that falls into the low hand off the beat: the
/// lowest that stands clear above the note it leaves. That is a D gracenote
/// leaving B or below and an E gracenote leaving C or D. Above those, only a G
/// gracenote clears it.
fn falling_grace(from: Pitch) -> Pitch {
    use Pitch::*;
    match from {
        LowG | LowA | B => D,
        C | D => E,
        _ => HighG,
    }
}

/// The gracenotes a march gives one note, given the note before it and the
/// beats in a bar.
fn march_graces(before: Option<Pitch>, note: &Note, meter: i64) -> Vec<Pitch> {
    use Pitch::*;
    let pitch = note.pitch;
    // A chanter cannot stop or tongue, so a repeated note has to be parted.
    // High A has nothing above it and is struck from below. A tune's first note
    // needs the same clean start.
    if before.is_none_or(|b| b == pitch) {
        return vec![cut(pitch)];
    }
    // The main beats, the first of the bar and the one halfway through it,
    // carry a march's weight.
    if note.at % (meter * UNITS_PER_BEAT / 2) == 0 {
        return match pitch {
            // the long drop from high G is the hardest attack the low notes have
            LowG | LowA => vec![HighG],
            // a D held for a beat is thrown
            D if note.units >= UNITS_PER_BEAT => THROW.to_vec(),
            _ => doubling(pitch),
        };
    }
    // Any other beat is marked with one gracenote. High A has none above it.
    if note.at % UNITS_PER_BEAT == 0 {
        return if pitch == HighA {
            Vec::new()
        } else {
            vec![cut(pitch)]
        };
    }
    // Off the beat a gracenote stays light, and only on a note long enough to
    // carry one that falls into the low hand. A note that rises, or a short
    // note after a dotted one, is played plain.
    match before {
        Some(from) if pitch < from && pitch <= C && note.units >= UNITS_PER_BEAT / 2 => {
            vec![falling_grace(from)]
        }
        _ => Vec::new(),
    }
}

/// Gives a plain melody in bars of `meter` beats its gracenotes.
fn grace_march(melody: &mut [Note], meter: i64) {
    let mut before = None;
    for note in melody {
        note.graces = march_graces(before, note, meter);
        before = Some(note.pitch);
    }
}

/// Every note of these strains in order, the lift first, with its gracenotes.
/// An unknown strain is refused by name.
pub fn notes(score: &Score, strains: &[String]) -> Result<Vec<Note>, String> {
    let mut pace = 100;
    let mut out = read_bar(score.lift, &mut pace)?;
    for id in strains {
        let strain = score
            .strain(id)
            .ok_or_else(|| format!("{} has no strain \"{id}\".", score.id))?;
        for bar in strain.bars {
            out.extend(read_bar(bar, &mut pace)?);
        }
    }
    if score.graced_by_rule {
        grace_march(&mut out, score.meter);
    }
    Ok(out)
}

/// What the chanter plays, from the lift's first note at `from_ms`, and when
/// the last note ends. `beat_ms` is one beat at a pace of 100. A gracenote
/// sits on the beat and the note it decorates sounds on through the rest of
/// its length, as a piper plays it.
pub fn steps(notes: &[Note], from_ms: i64, beat_ms: i64, grace_ms: i64) -> (Vec<Step>, i64) {
    let mut out = Vec::new();
    // in units of a percent of a unit of a millisecond, so a long tune does
    // not gather rounding
    let scale = 100 * UNITS_PER_BEAT;
    let mut at = from_ms * scale;
    for note in notes {
        let start = at / scale;
        for (k, pitch) in note.graces.iter().enumerate() {
            out.push(Step {
                at_ms: start + k as i64 * grace_ms,
                pitch: *pitch,
                grace: true,
            });
        }
        out.push(Step {
            at_ms: start + note.graces.len() as i64 * grace_ms,
            pitch: note.pitch,
            grace: false,
        });
        at += note.units * beat_ms * note.pace;
    }
    (out, at / scale)
}

#[cfg(test)]
mod tests {
    use super::Pitch::*;
    use super::*;

    fn all_strains(score: &Score) -> Vec<String> {
        score.strains.iter().map(|s| s.id.to_string()).collect()
    }

    fn keys(notes: &[Note]) -> Vec<&'static str> {
        notes.iter().map(|n| n.pitch.key()).collect()
    }

    #[test]
    fn every_bar_of_both_tunes_fills_its_meter() {
        for score in SCORES {
            let mut pace = 100;
            let lift: i64 = read_bar(score.lift, &mut pace)
                .unwrap()
                .iter()
                .map(|n| n.units)
                .sum();
            let full = score.meter * UNITS_PER_BEAT;
            for strain in score.strains {
                for (k, bar) in strain.bars.iter().enumerate() {
                    let units: i64 = read_bar(bar, &mut pace).unwrap().iter().map(|n| n.units).sum();
                    let last = k + 1 == strain.bars.len();
                    // a march's last bar leaves room for the lift
                    let short = last && units + lift == full;
                    assert!(
                        units == full || short,
                        "{} {} bar {k}: {units} of {full}",
                        score.id,
                        strain.id
                    );
                }
            }
        }
    }

    #[test]
    fn scotland_the_brave_runs_as_the_pipe_score_does() {
        // hand-read from the pipe score: the lift and the first four bars
        let n = notes(&SCOTLAND_THE_BRAVE, &["opening".to_string()]).unwrap();
        let expected = [
            "e", "la", "la", "b", "c", "la", "c", "e", "ha", "ha", "ha", "e", "c", "la", "d", "f", "d", "c",
            "e", "c", "la", "b", "e", "e", "f", "e", "d", "c", "b",
        ];
        assert_eq!(keys(&n), expected);
    }

    #[test]
    fn the_flowers_of_the_forest_runs_as_the_pipe_score_does() {
        // hand-read from the scores: the lift and the first two bars
        let n = notes(&FLOWERS_OF_THE_FOREST, &["opening".to_string()]).unwrap();
        let expected = [
            "e", "la", "la", "b", "la", "b", "c", "d", "e", "f", "hg", "f", "e", "d", "b", "d", "b", "la",
            "lg",
        ];
        assert_eq!(&keys(&n)[..expected.len()], expected);
    }

    #[test]
    fn both_tunes_stay_on_the_chanter_and_use_all_of_it_between_them() {
        let mut used = Vec::new();
        for score in SCORES {
            used.extend(
                notes(&score, &all_strains(&score))
                    .unwrap()
                    .iter()
                    .map(|n| n.pitch),
            );
        }
        for pitch in Pitch::ALL {
            assert!(used.contains(&pitch), "{pitch:?} is never played");
        }
    }

    #[test]
    fn the_pipes_never_repeat_a_note_without_a_gracenote() {
        for score in SCORES {
            let n = notes(&score, &all_strains(&score)).unwrap();
            for pair in n.windows(2) {
                let same = pair[0].pitch == pair[1].pitch;
                assert!(!same || !pair[1].graces.is_empty(), "{}: {:?}", score.id, pair[1]);
            }
        }
    }

    #[test]
    fn a_gracenote_is_never_the_note_it_leads_into() {
        for score in SCORES {
            for note in notes(&score, &all_strains(&score)).unwrap() {
                assert_ne!(note.graces.last(), Some(&note.pitch), "{}: {note:?}", score.id);
            }
        }
    }

    #[test]
    fn embellishments_play_the_notes_a_piper_plays() {
        let played = |text: &str| read_bar(text, &mut 100).unwrap().remove(0).graces;
        assert_eq!(played("gg la4"), [HighG]);
        assert_eq!(played("tg f4"), [HighA]);
        assert_eq!(played("eg la4"), [E]);
        assert_eq!(played("grp la4"), [LowG, D, LowG]);
        assert_eq!(played("thr d4"), [LowG, D, C]);
        assert_eq!(played("dbl lg4"), [HighG, LowG, D]);
        assert_eq!(played("dbl la4"), [HighG, LowA, D]);
        assert_eq!(played("dbl b4"), [HighG, B, D]);
        assert_eq!(played("dbl c4"), [HighG, C, D]);
        assert_eq!(played("dbl d4"), [HighG, D, E]);
        assert_eq!(played("dbl e4"), [HighG, E, F]);
        assert_eq!(played("dbl f4"), [HighG, F, HighG]);
        assert_eq!(played("dbl hg4"), [HighG, F]);
        assert_eq!(played("dbl ha4"), [HighA, HighG]);
    }

    /// The march rule on one note, `at` units into a bar of 4/4.
    fn ruled(before: Pitch, word: &str, at: i64) -> Vec<Pitch> {
        let mut note = read_bar(word, &mut 100).unwrap().remove(0);
        note.at = at;
        march_graces(Some(before), &note, 4)
    }

    #[test]
    fn the_rule_parts_a_repeated_note_with_what_can_cut_it() {
        assert_eq!(ruled(B, "b8", 12), [HighG]);
        assert_eq!(ruled(HighG, "hg8", 12), [HighA]);
        assert_eq!(ruled(HighA, "ha8", 12), [HighG]);
        // a repeat on a main beat takes the same, not a doubling
        assert_eq!(ruled(C, "c4", 48), [HighG]);
    }

    #[test]
    fn the_rule_starts_a_tune_on_a_gracenote() {
        let mut melody = read_bar("e8", &mut 100).unwrap();
        grace_march(&mut melody, 4);
        assert_eq!(melody[0].graces, [HighG]);
    }

    #[test]
    fn the_rule_doubles_the_main_beats() {
        assert_eq!(ruled(LowA, "c8", 0), [HighG, C, D]);
        assert_eq!(ruled(E, "b4", 48), [HighG, B, D]);
        assert_eq!(ruled(E, "ha4", 0), [HighA, HighG]);
    }

    #[test]
    fn the_rule_gives_the_low_notes_a_g_gracenote_on_a_main_beat() {
        assert_eq!(ruled(C, "la4", 0), [HighG]);
        assert_eq!(ruled(C, "lg4", 48), [HighG]);
    }

    #[test]
    fn the_rule_throws_a_d_held_for_a_beat_on_a_main_beat() {
        assert_eq!(ruled(LowA, "d4", 0), [LowG, D, C]);
        assert_eq!(ruled(LowA, "d8", 0), [HighG, D, E]);
        assert_eq!(ruled(LowA, "d4", 24), [HighG]);
    }

    #[test]
    fn the_rule_marks_a_lesser_beat_with_one_gracenote() {
        assert_eq!(ruled(C, "f4", 24), [HighG]);
        assert_eq!(ruled(C, "b8", 72), [HighG]);
        assert_eq!(ruled(E, "hg4", 24), [HighA]);
        assert!(ruled(E, "ha4", 24).is_empty());
    }

    #[test]
    fn the_rule_cuts_a_fall_off_the_beat_by_the_note_it_leaves() {
        assert_eq!(ruled(B, "la8", 12), [D]);
        assert_eq!(ruled(C, "la8", 12), [E]);
        assert_eq!(ruled(D, "b8", 12), [E]);
        assert_eq!(ruled(F, "c8", 12), [HighG]);
    }

    #[test]
    fn the_rule_leaves_other_notes_off_the_beat_plain() {
        assert!(ruled(LowA, "c8", 12).is_empty(), "a rise");
        assert!(
            ruled(HighA, "e8", 12).is_empty(),
            "a fall that stays in the top hand"
        );
        assert!(ruled(C, "la16", 18).is_empty(), "a short note");
    }

    #[test]
    fn scotland_the_brave_names_no_gracenote_in_its_bars() {
        let mut pace = 100;
        let bars = SCOTLAND_THE_BRAVE.strains.iter().flat_map(|s| s.bars);
        for bar in bars {
            let plain = read_bar(bar, &mut pace)
                .unwrap()
                .iter()
                .all(|n| n.graces.is_empty());
            assert!(plain, "{bar}");
        }
    }

    #[test]
    fn scotland_the_brave_takes_its_gracing_from_the_rule() {
        // worked by hand through the rule: the lift, then the first bar
        let n = notes(&SCOTLAND_THE_BRAVE, &["opening".to_string()]).unwrap();
        let graces: Vec<&[Pitch]> = n[..8].iter().map(|x| x.graces.as_slice()).collect();
        let expected: [&[Pitch]; 8] = [
            &[HighG],       // the lift, the tune's first note
            &[HighG],       // low A on the first beat
            &[HighG],       // low A again, parted
            &[],            // the short B
            &[HighG, C, D], // C on the third beat, doubled
            &[E],           // low A falling from C off the beat
            &[HighG],       // C on the fourth beat
            &[],            // E rising off the beat
        ];
        assert_eq!(graces, expected);
    }

    #[test]
    fn the_lament_keeps_the_gracing_written_in_its_bars() {
        let n = notes(&FLOWERS_OF_THE_FOREST, &["opening".to_string()]).unwrap();
        assert_eq!(n[2].graces, [LowG, D, LowG], "a grip, which the rule never plays");
    }

    #[test]
    fn lengths_read_in_units_of_a_beat() {
        let units = |text: &str| read_bar(text, &mut 100).unwrap()[0].units;
        assert_eq!(units("la4"), 24);
        assert_eq!(units("la4."), 36);
        assert_eq!(units("la8"), 12);
        assert_eq!(units("b8.."), 21);
        assert_eq!(units("b16."), 9);
        assert_eq!(units("c32"), 3);
        assert_eq!(units("e16t"), 4);
    }

    #[test]
    fn a_bad_word_in_a_bar_is_refused_by_name() {
        let e = read_bar("zz la4", &mut 100).unwrap_err();
        assert!(e.contains("zz"), "{e}");
        assert!(read_bar("q4", &mut 100).unwrap_err().contains("q4"));
        assert!(read_bar("la5", &mut 100).unwrap_err().contains("la5"));
        assert!(read_bar("la4x", &mut 100).unwrap_err().contains("la4x"));
        assert!(read_bar("gg", &mut 100).unwrap_err().contains("gg"));
        let e = notes(&SCOTLAND_THE_BRAVE, &["z".to_string()]).unwrap_err();
        assert!(e.contains("\"z\""), "{e}");
    }

    #[test]
    fn steps_put_a_note_after_its_gracenotes_and_the_next_on_the_beat() {
        let n = read_bar("gg la4 b4", &mut 100).unwrap();
        let (s, end) = steps(&n, 1000, 600, 30);
        let at = |k: usize| (s[k].at_ms, s[k].pitch, s[k].grace);
        assert_eq!(at(0), (1000, HighG, true));
        assert_eq!(at(1), (1030, LowA, false));
        assert_eq!(at(2), (1600, B, false));
        assert_eq!(end, 2200);
    }

    #[test]
    fn pace_stretches_the_notes_it_covers() {
        let n = read_bar("~150 la4 ~50 b4", &mut 100).unwrap();
        let (s, end) = steps(&n, 0, 1000, 30);
        assert_eq!(s[1].at_ms, 1500);
        assert_eq!(end, 2000);
    }

    #[test]
    fn the_march_keeps_strict_time_and_the_lament_is_played_freely() {
        let span = |score: &Score, strain: &str| -> Vec<i64> {
            let mut pace = 100;
            let bars = score.strain(strain).unwrap().bars;
            let bar_ms = |bar: &&str, pace: &mut i64| {
                let n = read_bar(bar, pace).unwrap();
                n.iter().map(|x| x.units * x.pace).sum::<i64>()
            };
            bars.iter().map(|b| bar_ms(b, &mut pace)).collect()
        };
        let march = span(&SCOTLAND_THE_BRAVE, "opening");
        assert!(march.windows(2).all(|p| p[0] == p[1]), "{march:?}");
        let lament = span(&FLOWERS_OF_THE_FOREST, "opening");
        assert!(lament.windows(2).any(|p| p[0] != p[1]), "{lament:?}");
    }
}
