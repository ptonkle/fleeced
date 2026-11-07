//! Sound: what plays, how it is built, how loud and from where. There are no
//! audio files. Every sound is a recipe in `data/sound.json`, and the two win
//! tunes are notes in `tunes.rs`. The core compiles both into plain schedules
//! (the shape of Honey Hole's, with periodic waves, several filters and
//! positions added), and the page's small synth (`web/audio.js`) replays them
//! with Web Audio and computes nothing. `tools/sound/render.py` replays the
//! same schedules offline, so a person can listen without running the game.
//!
//! All numbers here are whole thousandths ("milli"): the core does no
//! floating-point math (BUILDER-SPEC D2). 0.4 in the file is 400 here, and
//! the page is sent decimals written back from thousandths.

use serde_json::{json, Map, Number, Value};

use crate::fx::{self, Fx};
use crate::rng::Rng;
use crate::tunes::{self, Pitch, Step};

const SOUND_JSON: &str = include_str!("../../../data/sound.json");

/// One whole unit, in thousandths.
pub const MILLI: i64 = 1000;

const LARGEST: i64 = 1_000_000_000;

/// A JSON number as whole thousandths: 0.25 -> 250, 440 -> 440_000.
pub fn parse_milli(v: &Value, what: &str) -> Result<i64, String> {
    let Value::Number(n) = v else {
        return Err(format!("{what} is not a number."));
    };
    let text = n.to_string();
    if text.contains(['e', 'E']) {
        return Err(format!("{what} ({text}) must be a plain decimal."));
    }
    let (negative, body) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.as_str()),
    };
    let (whole, frac) = body.split_once('.').unwrap_or((body, ""));
    let frac = frac.trim_end_matches('0');
    if frac.len() > 3 {
        return Err(format!("{what} ({text}) has more than three decimal places."));
    }
    let whole: i64 = whole
        .parse()
        .ok()
        .filter(|w| *w <= LARGEST)
        .ok_or_else(|| format!("{what} ({text}) is too large."))?;
    let mut thousandths = 0;
    for (k, c) in frac.chars().enumerate() {
        thousandths += c.to_digit(10).unwrap_or(0) as i64 * [100, 10, 1][k];
    }
    let v = whole * MILLI + thousandths;
    Ok(if negative { -v } else { v })
}

/// Thousandths as a JSON decimal: 250 -> 0.25.
pub fn dec(milli: i64) -> Value {
    let sign = if milli < 0 { "-" } else { "" };
    let a = milli.unsigned_abs();
    let text = format!("{sign}{}.{:03}", a / MILLI as u64, a % MILLI as u64);
    Value::Number(
        text.parse::<Number>()
            .expect("a written decimal is a JSON number"),
    )
}

/// `a * b` where both are thousandths, in thousandths.
pub fn mul(a: i64, b: i64) -> i64 {
    a * b / MILLI
}

// ---- reading the file ----

fn get<'a>(o: &'a Value, key: &str, at: &str) -> Result<&'a Value, String> {
    o.get(key).ok_or_else(|| format!("{at} has no {key}."))
}

fn num(o: &Value, key: &str, at: &str) -> Result<i64, String> {
    parse_milli(get(o, key, at)?, &format!("{at} {key}"))
}

fn num_or(o: &Value, key: &str, at: &str, default: i64) -> Result<i64, String> {
    o.get(key)
        .map_or(Ok(default), |v| parse_milli(v, &format!("{at} {key}")))
}

/// A whole number of milliseconds or the like, as it is written in the file.
fn whole(o: &Value, key: &str, at: &str) -> Result<i64, String> {
    Ok(num(o, key, at)? / MILLI)
}

fn whole_or(o: &Value, key: &str, at: &str, default: i64) -> Result<i64, String> {
    Ok(num_or(o, key, at, default * MILLI)? / MILLI)
}

fn text<'a>(o: &'a Value, key: &str, at: &str) -> Result<&'a str, String> {
    get(o, key, at)?
        .as_str()
        .ok_or_else(|| format!("{at} {key} is not text."))
}

fn list<'a>(o: &'a Value, key: &str, at: &str) -> Result<&'a [Value], String> {
    get(o, key, at)?
        .as_array()
        .map(|v| v.as_slice())
        .ok_or_else(|| format!("{at} {key} is not a list."))
}

fn numbers(o: &Value, key: &str, at: &str) -> Result<Vec<i64>, String> {
    list(o, key, at)?
        .iter()
        .map(|v| parse_milli(v, &format!("{at} {key}")))
        .collect()
}

// ---- schedules ----

/// A parameter's schedule, as the page's AudioParam methods take it.
#[derive(Debug, Clone, Default, PartialEq)]
struct Curve(Vec<Value>);

impl Curve {
    fn point(&mut self, method: &str, at_ms: i64, milli: i64) -> &mut Curve {
        self.0
            .push(json!({"m": method, "t": dec(at_ms), "v": dec(milli)}));
        self
    }
    fn set(&mut self, at_ms: i64, milli: i64) -> &mut Curve {
        self.point("setValueAtTime", at_ms, milli)
    }
    fn ramp(&mut self, at_ms: i64, milli: i64) -> &mut Curve {
        self.point("linearRampToValueAtTime", at_ms, milli)
    }
    fn fall(&mut self, at_ms: i64, milli: i64) -> &mut Curve {
        self.point("exponentialRampToValueAtTime", at_ms, milli)
    }
    /// Falls toward `milli` from `at_ms`, a third of the way left every
    /// `over_ms`, until the next point.
    fn settle(&mut self, at_ms: i64, milli: i64, over_ms: i64) -> &mut Curve {
        let point = json!({"m": "setTargetAtTime", "t": dec(at_ms), "v": dec(milli), "tc": dec(over_ms)});
        self.0.push(point);
        self
    }
}

/// A list of filters with fixed or swept frequencies. `from` and `to` bound a
/// sweep; `pitch` scales a filter marked `follow`.
fn filters(l: &Value, at: &str, from: i64, to: i64, pitch: i64) -> Result<Vec<Value>, String> {
    let Some(raw) = l.get("filters") else {
        return Ok(Vec::new());
    };
    let raw = raw
        .as_array()
        .ok_or_else(|| format!("{at} filters is not a list."))?;
    let mut out = Vec::with_capacity(raw.len());
    for f in raw {
        let scale = if f.get("follow").is_some() { pitch } else { MILLI };
        let mut freq = Curve::default();
        freq.set(from, mul(num(f, "freq", at)?, scale));
        if f.get("to_freq").is_some() {
            freq.fall(to, mul(num(f, "to_freq", at)?, scale));
        }
        out.push(json!({
            "type": text(f, "type", at)?,
            "freq": freq.0,
            "q": dec(num_or(f, "q", at, 0)?),
            "gain": dec(num_or(f, "gain", at, 0)?),
        }));
    }
    Ok(out)
}

/// `[{rate, cents}]` and the like, passed through as decimals.
fn rates(l: &Value, key: &str, depth: &str, at: &str) -> Result<Vec<Value>, String> {
    let Some(raw) = l.get(key) else {
        return Ok(Vec::new());
    };
    let raw = raw
        .as_array()
        .ok_or_else(|| format!("{at} {key} is not a list."))?;
    raw.iter()
        .map(|w| {
            Ok(json!({
                "rate": dec(num(w, "rate", at)?),
                depth: dec(num(w, depth, at)?),
            }))
        })
        .collect()
}

/// A harmonic table as a periodic wave: sine terms, the first entry unused.
fn periodic(harmonics: &[i64]) -> Value {
    let mut imag = vec![dec(0)];
    imag.extend(harmonics.iter().map(|a| dec(*a)));
    json!({"real": vec![dec(0); imag.len()], "imag": imag})
}

const WAVES: [&str; 5] = ["sine", "square", "sawtooth", "triangle", "noise"];

/// The rate of the noise file.
const SAMPLE_RATE: i64 = 44_100;

fn pair(v: &Value, at: &str) -> Result<(i64, i64), String> {
    match v.as_array().map(|p| p.as_slice()) {
        Some([a, b]) => Ok((parse_milli(a, at)?, parse_milli(b, at)?)),
        _ => Err(format!("{at} wants a pair of numbers.")),
    }
}

/// A bell's level: one voice that keeps ringing while a clapper strikes it
/// again and again. Each strike jumps to its level and rings down. Returns
/// when the last strike has died away.
fn struck(g: &mut Curve, l: &Value, at: &str, start: i64, peak: i64) -> Result<i64, String> {
    let ring = whole(l, "ring_ms", at)?;
    let mut last = start;
    for row in list(l, "strikes", at)? {
        let (ms, level) = pair(row, at)?;
        last = start + ms / MILLI;
        g.set(last, mul(peak, level)).settle(last, 0, ring);
    }
    Ok(last + whole(l, "tail_ms", at)?)
}

// ---- the bank ----

/// `data/sound.json`, read and checked.
#[derive(Debug, Clone, PartialEq)]
pub struct Bank {
    file: Value,
}

/// The sounds, read from `data/sound.json`.
pub fn bank() -> Result<Bank, String> {
    let file: Value = serde_json::from_str(SOUND_JSON).map_err(|e| format!("data/sound.json: {e}"))?;
    Ok(Bank { file })
}

impl Bank {
    /// One cue as the page plays it: every layer's schedule, at `gain` and
    /// `pitch` (thousandths; 1000 plays it as written).
    pub fn cue(&self, id: &str, gain: i64, pitch: i64) -> Result<Value, String> {
        let cue = self.find_cue(id)?;
        let at = format!("cue {id}");
        let mut layers = Vec::new();
        let mut length = 0;
        for l in list(cue, "layers", &at)? {
            let (layer, end) = self.layer(l, &at, gain, pitch)?;
            layers.push(layer);
            length = length.max(end);
        }
        Ok(json!({"id": id, "length": dec(length), "layers": layers}))
    }

    fn find_cue(&self, id: &str) -> Result<&Value, String> {
        list(&self.file, "cues", "sound.json")?
            .iter()
            .find(|c| c.get("id").and_then(Value::as_str) == Some(id))
            .ok_or_else(|| format!("No cue \"{id}\"."))
    }

    fn layer(&self, l: &Value, at: &str, gain: i64, pitch: i64) -> Result<(Value, i64), String> {
        let start = whole_or(l, "at_ms", at, 0)?;
        let peak = mul(num(l, "gain", at)?, gain);

        let mut g = Curve::default();
        let end = if l.get("strikes").is_some() {
            struck(&mut g, l, at, start, peak)?
        } else {
            let attack = whole_or(l, "attack_ms", at, 2)?;
            let hold = whole_or(l, "hold_ms", at, 0)?;
            let end = start + attack + hold + whole_or(l, "release_ms", at, 40)?;
            let quiet = num(&self.file, "silence", "sound.json")?;
            g.set(start, 0)
                .ramp(start + attack, peak)
                .set(start + attack + hold, peak);
            g.fall(end, quiet.min(peak.max(1)));
            end
        };

        let mut out = Map::new();
        out.insert("gain".into(), Value::Array(g.0));
        out.insert("start".into(), dec(start));
        out.insert("stop".into(), dec(end));
        self.layer_source(&mut out, l, at, start, end, pitch)?;
        Ok((Value::Object(out), end))
    }

    /// The parts of a layer a one-shot and a loop share: its wave, pitch,
    /// filters, wobble and throb.
    fn layer_source(
        &self,
        out: &mut Map<String, Value>,
        l: &Value,
        at: &str,
        start: i64,
        end: i64,
        pitch: i64,
    ) -> Result<(), String> {
        let wave = text(l, "wave", at)?;
        if !WAVES.contains(&wave) {
            return Err(format!("{at} has wave \"{wave}\"."));
        }
        out.insert("wave".into(), json!(wave));
        if wave != "noise" {
            let mut f = Curve::default();
            f.set(start, mul(num(l, "freq", at)?, pitch));
            if l.get("to_freq").is_some() {
                let to = mul(num(l, "to_freq", at)?, pitch);
                let bend = start + whole_or(l, "glide_ms", at, end - start)?;
                if l.get("glide").and_then(Value::as_str) == Some("linear") {
                    f.ramp(bend, to);
                } else {
                    f.fall(bend, to);
                }
            }
            if let Some(bends) = l.get("bends").and_then(Value::as_array) {
                for b in bends {
                    let (ms, hz) = pair(b, at)?;
                    f.ramp(start + ms / MILLI, mul(hz, pitch));
                }
            }
            out.insert("freq".into(), Value::Array(f.0));
        }
        out.insert("filters".into(), Value::Array(filters(l, at, start, end, pitch)?));
        out.insert("wobble".into(), Value::Array(rates(l, "wobble", "cents", at)?));
        out.insert("throb".into(), Value::Array(rates(l, "throb", "depth", at)?));
        Ok(())
    }

    /// The cues in a group, in file order.
    pub fn group(&self, name: &str) -> Result<Vec<String>, String> {
        let cues = list(&self.file, "cues", "sound.json")?;
        Ok(cues
            .iter()
            .filter(|c| c.get("group").and_then(Value::as_str) == Some(name))
            .filter_map(|c| c.get("id").and_then(Value::as_str).map(str::to_string))
            .collect())
    }

    /// How far a cue carries.
    pub fn reach(&self, id: &str) -> Result<Reach, String> {
        let cue = self.find_cue(id)?;
        let name = text(cue, "reach", id)?;
        let rows = get(&self.file, "falloff", "sound.json")?;
        let row = get(get(rows, "reach", "falloff")?, name, id)?;
        Ok(Reach {
            near: num(row, "near_m", name)?,
            far: num(row, "far_m", name)?,
            floor: num(row, "floor", name)?,
            far_hz: num_or(row, "far_hz", name, num(rows, "far_hz", "falloff")?)?,
        })
    }

    /// A loop as the page plays it: its layers at their written levels. The
    /// page starts it, follows its level and fades it out.
    pub fn looped(&self, id: &str, length_ms: i64) -> Result<Value, String> {
        let at = format!("loop {id}");
        let mut layers = Vec::new();
        for l in list(self.find_cue(id)?, "layers", &at)? {
            let mut g = Curve::default();
            g.set(0, num(l, "gain", &at)?);
            let mut out = Map::new();
            out.insert("gain".into(), Value::Array(g.0));
            out.insert("start".into(), dec(0));
            out.insert("stop".into(), dec(length_ms));
            self.layer_source(&mut out, l, &at, 0, length_ms, MILLI)?;
            layers.push(Value::Object(out));
        }
        Ok(json!({"id": id, "length": dec(length_ms), "layers": layers}))
    }

    /// A cue as the test page and the offline tools hear it: a loop runs for
    /// a few seconds.
    pub fn preview(&self, id: &str) -> Result<Value, String> {
        if Bank::is_loop(self.find_cue(id)?) {
            let ms = whole(get(&self.file, "mixer", "sound.json")?, "loop_preview_ms", id)?;
            self.looped(id, ms)
        } else {
            self.cue(id, MILLI, MILLI)
        }
    }

    fn is_loop(cue: &Value) -> bool {
        cue.get("loop").is_some()
    }

    /// What `startAudio` takes, besides the noise.
    /// The share of full volume a match plays at, in thousandths.
    pub fn loudest_in_play(&self) -> Result<i64, String> {
        let m = get(&self.file, "mixer", "sound.json")?;
        num(m, "loudest_in_play", "mixer")
    }

    pub fn setup(&self) -> Result<Value, String> {
        let m = get(&self.file, "mixer", "sound.json")?;
        let limiter = get(m, "limiter", "mixer")?;
        let mut loops = Vec::new();
        for c in list(&self.file, "cues", "sound.json")?
            .iter()
            .filter(|c| Bank::is_loop(c))
        {
            loops.push(self.looped(text(c, "id", "cue")?, 0)?);
        }
        Ok(json!({
            "loops": loops,
            "panning": text(m, "panning", "mixer")?,
            "smooth": dec(whole(m, "smooth_ms", "mixer")?),
            "fade": dec(whole(m, "fade_ms", "mixer")?),
            "limiter": {
                "threshold": dec(num(limiter, "threshold_db", "limiter")?),
                "knee": dec(num(limiter, "knee_db", "limiter")?),
                "ratio": dec(num(limiter, "ratio", "limiter")?),
                "attack": dec(whole(limiter, "attack_ms", "limiter")?),
                "release": dec(whole(limiter, "release_ms", "limiter")?),
            },
        }))
    }

    /// Every sound and tune with its label, for the sound test page.
    pub fn listing(&self) -> Result<Value, String> {
        let row = |c: &Value| -> Result<Value, String> {
            Ok(json!({"id": text(c, "id", "cue")?, "label": text(c, "label", "cue")?}))
        };
        let cues = list(&self.file, "cues", "sound.json")?;
        let once: Result<Vec<Value>, String> = cues.iter().filter(|c| !Bank::is_loop(c)).map(row).collect();
        let loops: Result<Vec<Value>, String> = cues.iter().filter(|c| Bank::is_loop(c)).map(row).collect();
        let tunes: Result<Vec<Value>, String> =
            list(&self.file, "tunes", "sound.json")?.iter().map(row).collect();
        Ok(json!({
            "words": get(&self.file, "test_page", "sound.json")?,
            "tunes": tunes?,
            "cues": once?,
            "loops": loops?,
            "flocks": numbers(get(&self.file, "mixer", "sound.json")?, "test_flocks", "mixer")?
                .iter()
                .map(|n| n / MILLI)
                .collect::<Vec<i64>>(),
        }))
    }

    /// The noise every noise layer plays: from a seed, so it is the same on
    /// every machine.
    pub fn noise_samples(&self) -> Result<Vec<i16>, String> {
        let row = get(&self.file, "noise", "sound.json")?;
        let count = whole(row, "samples", "noise")?;
        let mut rng = Rng::new(whole(row, "seed", "noise")? as u64);
        Ok((0..count).map(|_| (rng.next_u32() >> 16) as u16 as i16).collect())
    }

    /// The noise as a 16-bit mono WAV file, which the page decodes.
    pub fn noise_wav(&self) -> Result<Vec<u8>, String> {
        let pcm = self.noise_samples()?;
        let rate = SAMPLE_RATE as u32;
        let data_len = (pcm.len() * 2) as u32;
        let mut out = Vec::with_capacity(44 + data_len as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&rate.to_le_bytes());
        out.extend_from_slice(&(rate * 2).to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for s in &pcm {
            out.extend_from_slice(&s.to_le_bytes());
        }
        Ok(out)
    }

    /// The tune's description in the file.
    fn tune_row(&self, id: &str) -> Result<&Value, String> {
        list(&self.file, "tunes", "sound.json")?
            .iter()
            .find(|c| c.get("id").and_then(Value::as_str) == Some(id))
            .ok_or_else(|| format!("No tune \"{id}\"."))
    }

    fn pipes(&self) -> Result<&Value, String> {
        get(&self.file, "pipes", "sound.json")
    }

    /// The frequency of a chanter note, in milli-hertz: low A times the
    /// note's ratio in the pipe scale.
    pub fn pitch_hz(&self, pitch: Pitch) -> Result<i64, String> {
        let pipes = self.pipes()?;
        let ratio = numbers(get(pipes, "scale", "pipes")?, pitch.key(), "pipes scale")?;
        Ok(num(pipes, "low_a_hz", "pipes")? * ratio[0] / ratio[1])
    }

    /// The steps the chanter plays, the strike-in's E first, and the end of
    /// its last note.
    fn tune_steps(&self, id: &str) -> Result<(Vec<Step>, i64), String> {
        let row = self.tune_row(id)?;
        let at = format!("tune {id}");
        let score = tunes::score(id).ok_or_else(|| format!("No score for \"{id}\"."))?;
        let strains: Vec<String> = list(row, "strains", &at)?
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect();
        let notes = tunes::notes(&score, &strains)?;
        let strike = get(self.pipes()?, "strike", "pipes")?;
        let enter = whole(strike, "chanter_at_ms", "strike")?;
        let tune_at = whole(strike, "tune_at_ms", "strike")?;
        let (mut steps, end) = tunes::steps(
            &notes,
            tune_at,
            whole(row, "beat_ms", &at)?,
            whole(row, "grace_ms", &at)?,
        );
        // the chanter strikes in on E and holds it until the tune starts
        let intro = Step {
            at_ms: enter,
            pitch: Pitch::E,
            grace: false,
        };
        steps.insert(0, intro);
        Ok((steps, end + whole_or(row, "last_note_ms", &at, 0)?))
    }

    /// The tune as the chanter plays it, for the checks: each step's start in
    /// seconds, its note and whether it is a gracenote.
    pub fn score(&self, id: &str) -> Result<Value, String> {
        let (steps, end) = self.tune_steps(id)?;
        let mut rows = Vec::with_capacity(steps.len());
        for s in &steps {
            rows.push(json!({
                "t": dec(s.at_ms),
                "note": s.pitch.key(),
                "hz": dec(self.pitch_hz(s.pitch)?),
                "grace": s.grace,
            }));
        }
        let grace = whole(self.tune_row(id)?, "grace_ms", id)?;
        Ok(json!({"id": id, "end": dec(end), "grace": dec(grace), "steps": rows}))
    }

    /// A drone's layer: it strikes in flat, rises to pitch, holds, and when
    /// the bag stops it dies away falling.
    fn drone(&self, row: &Value, end: i64, at: &str) -> Result<Value, String> {
        let pipes = self.pipes()?;
        let strike = get(pipes, "strike", "pipes")?;
        let hz = num(pipes, "low_a_hz", "pipes")? * MILLI / num(row, "below_low_a", at)?;
        let offset = num_or(row, "cents", at, 0)?;
        let at_ms = whole_or(row, "at_ms", at, 0)?;
        let rise = at_ms + whole(strike, "rise_ms", "strike")?;
        let flat = num(strike, "flat_cents", "strike")?;
        let die = end + whole(strike, "die_ms", "strike")?;
        let sag = num(strike, "sag_cents", "strike")?;
        let level = num(row, "gain", at)?;

        let mut gain = Curve::default();
        gain.set(at_ms, 0)
            .ramp(at_ms + whole(strike, "attack_ms", "strike")?, level);
        gain.set(end, level).ramp(die, 0);
        let mut detune = Curve::default();
        detune
            .set(at_ms, offset - flat)
            .ramp(rise, offset)
            .set(end, offset)
            .ramp(die, offset - sag);
        let mut freq = Curve::default();
        freq.set(at_ms, hz);
        Ok(json!({
            "wave": "periodic",
            "periodic": periodic(&numbers(row, "harmonics", at)?),
            "freq": freq.0,
            "detune": detune.0,
            "gain": gain.0,
            "filters": filters(row, at, at_ms, die, MILLI)?,
            "wobble": rates(row, "wobble", "cents", at)?,
            "throb": [],
            "start": dec(at_ms),
            "stop": dec(die),
        }))
    }

    /// A win tune as the page plays it: the drones under the whole of it, and
    /// the chanter's steps over them.
    pub fn tune(&self, id: &str) -> Result<Value, String> {
        let at = format!("tune {id}");
        let row = self.tune_row(id)?;
        let pipes = self.pipes()?;
        let strike = get(pipes, "strike", "pipes")?;
        let chanter = get(pipes, "chanter", "pipes")?;
        let (steps, end) = self.tune_steps(id)?;
        let enter = steps[0].at_ms;
        let cut = end + whole(strike, "cut_ms", "strike")?;

        let mut layers = Vec::new();
        for drone in list(pipes, "drones", "pipes")? {
            layers.push(self.drone(drone, end, &at)?);
        }

        let mut freq = Curve::default();
        for s in &steps {
            freq.set(s.at_ms, self.pitch_hz(s.pitch)?);
        }
        let level = num(chanter, "gain", "chanter")?;
        let mut gain = Curve::default();
        gain.set(enter, 0)
            .ramp(enter + whole(strike, "attack_ms", "strike")?, level);
        gain.set(end, level).ramp(cut, 0);
        let mut detune = Curve::default();
        detune.set(enter, -num(strike, "chanter_flat_cents", "strike")?);
        detune.ramp(enter + whole(strike, "chanter_rise_ms", "strike")?, 0);
        layers.push(json!({
            "wave": "periodic",
            "periodic": periodic(&numbers(chanter, "harmonics", "chanter")?),
            "freq": freq.0,
            "detune": detune.0,
            "gain": gain.0,
            "filters": filters(chanter, "chanter", enter, cut, MILLI)?,
            "wobble": rates(chanter, "wobble", "cents", "chanter")?,
            "throb": [],
            "start": dec(enter),
            "stop": dec(cut),
        }));

        let echo = get(pipes, "echo", "pipes")?;
        let mut taps = Vec::new();
        for t in list(echo, "taps", "echo")? {
            taps.push(json!({"delay": dec(whole(t, "ms", "echo")?), "gain": dec(num(t, "gain", "echo")?)}));
        }
        let die = end + whole(strike, "die_ms", "strike")?;
        Ok(json!({
            "id": id,
            "label": text(row, "label", &at)?,
            "length": dec(die + whole(echo, "tail_ms", "echo")?),
            "layers": layers,
            "echo": {"taps": taps, "filter": dec(num(echo, "filter_hz", "echo")?), "q": dec(num(echo, "filter_q", "echo")?)},
        }))
    }

    /// Every sound, compiled, for the offline tools.
    pub fn dump(&self) -> Result<String, String> {
        let mut tunes = Vec::new();
        let mut scores = Vec::new();
        for row in list(&self.file, "tunes", "sound.json")? {
            let id = text(row, "id", "tune")?;
            tunes.push(self.tune(id)?);
            scores.push(self.score(id)?);
        }
        let mut cues = Vec::new();
        for c in list(&self.file, "cues", "sound.json")? {
            cues.push(self.preview(text(c, "id", "cue")?)?);
        }
        let noise = self.noise_samples()?;
        Ok(json!({"tunes": tunes, "scores": scores, "cues": cues, "noise": noise}).to_string())
    }
}

// ---- hearing: distance, direction and the flock ----

/// A point in the world, in Q16.16 meters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point {
    pub x: Fx,
    pub y: Fx,
    pub z: Fx,
}

/// What the flock tells the mixer about one sheep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeardSheep {
    pub at: Point,
    pub lamb: bool,
    pub moving: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gait {
    Still,
    Run,
    Sprint,
}

/// How far a kind of sound carries: thousandths of a meter, the level it never
/// falls under, and how dull it is at the far end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reach {
    pub near: i64,
    pub far: i64,
    pub floor: i64,
    pub far_hz: i64,
}

/// The level of a sound at `distance` (thousandths of a meter): full inside
/// `near`, then one over distance, tapering to `floor` at `far`.
pub fn falloff(distance: i64, reach: Reach) -> i64 {
    if distance <= reach.near {
        return MILLI;
    }
    if distance >= reach.far {
        return reach.floor;
    }
    let law = reach.near * MILLI / distance;
    let taper = (reach.far - distance) * MILLI / (reach.far - reach.near);
    mul(law, taper).max(reach.floor)
}

/// Main volume percent as the master gain: on a square, as ears hear it.
pub fn master_gain(percent: i64) -> i64 {
    let p = percent.clamp(0, 100);
    p * p * MILLI / 10_000
}

/// The main gain in a match: `master_gain` under the mixer's `loudest_in_play`,
/// so a full slider plays that share of the test page's level.
pub fn play_gain(percent: i64, loudest: i64) -> i64 {
    master_gain(percent) * loudest / MILLI
}

/// The tune that plays when this team wins.
pub fn win_tune(bank: &Bank, sheepdogs_won: bool) -> Result<String, String> {
    let row = get(&bank.file, "win_tunes", "sound.json")?;
    let key = if sheepdogs_won { "sheepdogs" } else { "wolves" };
    Ok(text(row, key, "win_tunes")?.to_string())
}

/// Thousandths of a meter between two points.
fn distance(a: Point, b: Point) -> i64 {
    to_milli(fx::length(fx::length(a.x - b.x, a.y - b.y), a.z - b.z) as i64)
}

/// A Q16.16 length in thousandths.
fn to_milli(v: i64) -> i64 {
    (v * MILLI) >> fx::SHIFT
}

/// A long sound whose level and direction keep following its source.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Tracked {
    key: String,
    who: i64,
    at: Point,
    reach: Reach,
    ends: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Tuning {
    max_baas: usize,
    baa_every: i64,
    baa_spread: i64,
    baa_tries: u32,
    baa_full_flock: i64,
    lamb_share: u32,
    audible: i64,
    hooves_every: i64,
    hooves_full: i64,
    hooves_gain: i64,
    run_step: i64,
    sprint_step: i64,
    pant: i64,
    open_hz: i64,
    noise_ms: i64,
}

const SEATS: usize = 8;
/// How many recent events the mixer remembers, so a rollback's replay of an
/// event does not play it twice.
const REMEMBERED: usize = 96;
const TICKS_PER_SECOND: i64 = 60;

/// Decides what is heard, frame by frame. It lives outside the world and rolls
/// nothing back: its chance is its own and never touches the simulation's.
///
/// Each tick the game calls `listen`, `flock`, `mover` for each seat, `event`
/// for what happened and `source` for a tracked sound's moving source, then
/// hands `frame`'s result to the page's `playFrame`.
#[derive(Debug, Clone)]
pub struct Mixer {
    bank: Bank,
    tuning: Tuning,
    rng: Rng,
    ear: Point,
    /// the way the listener faces on the ground, x and z
    facing: (Fx, Fx),
    pending: Vec<Value>,
    seen: Vec<(i64, i64, String)>,
    /// the tick each running baa ends
    baas: Vec<i64>,
    next_baa: i64,
    hooves: Option<(i64, Point)>,
    steps: [i64; SEATS],
    pants: [i64; SEATS],
    tracked: Vec<Tracked>,
}

impl Mixer {
    pub fn new(bank: Bank, seed: u64) -> Result<Mixer, String> {
        let m = get(&bank.file, "mixer", "sound.json")?;
        let f = get(&bank.file, "falloff", "sound.json")?;
        let at = "mixer";
        let tuning = Tuning {
            max_baas: whole(m, "max_baas", at)? as usize,
            baa_every: whole(m, "baa_every_ticks", at)?,
            baa_spread: whole(m, "baa_spread_ticks", at)?,
            baa_tries: whole(m, "baa_tries", at)? as u32,
            baa_full_flock: whole(m, "baa_full_flock", at)?,
            lamb_share: whole(m, "lamb_share_percent", at)? as u32,
            audible: num(m, "audible", at)?,
            hooves_every: whole(m, "hooves_every_ticks", at)?,
            hooves_full: whole(m, "hooves_full", at)?,
            hooves_gain: num(m, "hooves_gain", at)?,
            run_step: whole(m, "run_step_ticks", at)?,
            sprint_step: whole(m, "sprint_step_ticks", at)?,
            pant: whole(m, "pant_ticks", at)?,
            open_hz: num(f, "open_hz", "falloff")?,
            noise_ms: whole(get(&bank.file, "noise", "sound.json")?, "samples", "noise")? * MILLI
                / SAMPLE_RATE,
        };
        Ok(Mixer {
            bank,
            tuning,
            rng: Rng::new(seed),
            ear: Point::default(),
            facing: (0, -fx::ONE),
            pending: Vec::new(),
            seen: Vec::new(),
            baas: Vec::new(),
            next_baa: 0,
            hooves: None,
            steps: [0; SEATS],
            pants: [0; SEATS],
            tracked: Vec::new(),
        })
    }

    /// Where the listener stands and the way it faces on the ground (any
    /// length). The camera is the listener.
    pub fn listen(&mut self, at: Point, facing_x: Fx, facing_z: Fx) {
        self.ear = at;
        self.facing = (facing_x, facing_z);
    }

    /// Where a sound sits for the page: level, dullness and a position with
    /// the listener at the origin facing -z, x to its right and y up.
    fn place(&self, at: Point, reach: Reach) -> Value {
        let (fx_, fz) = (self.facing.0 as i64, self.facing.1 as i64);
        let size = (fx::length(self.facing.0, self.facing.1) as i64).max(1);
        let (dx, dz) = ((at.x - self.ear.x) as i64, (at.z - self.ear.z) as i64);
        let right = (dx * -fz + dz * fx_) / size;
        let ahead = (dx * fx_ + dz * fz) / size;
        let up = (at.y - self.ear.y) as i64;
        let d = distance(at, self.ear);
        let t = d.min(reach.far);
        let muffle = self.tuning.open_hz - (self.tuning.open_hz - reach.far_hz) * t / reach.far.max(1);
        json!({
            "gain": dec(falloff(d, reach)),
            "muffle": dec(muffle),
            "pos": [dec(to_milli(right)), dec(to_milli(up)), dec(to_milli(-ahead))],
        })
    }

    /// A cue by id or group name, picked at random from its group, at a
    /// random pitch within the row's spread; `pitch` then scales it. An
    /// unknown name is an error.
    fn pick(&mut self, name: &str, pitch: i64) -> Result<(String, i64), String> {
        let found = self.bank.group(name)?;
        let id = if found.is_empty() {
            self.bank.find_cue(name)?;
            name.to_string()
        } else {
            found[self.rng.below(found.len() as u32) as usize].clone()
        };
        let spread = num_or(self.bank.find_cue(&id)?, "pitch_spread", &id, 0)?;
        let chance = self.rng.below((2 * spread + 1) as u32) as i64;
        Ok((id, mul(pitch, MILLI - spread + chance)))
    }

    fn queue(
        &mut self,
        name: &str,
        who: i64,
        at: Option<Point>,
        pitch: i64,
        tick: i64,
    ) -> Result<bool, String> {
        let (id, pitch) = self.pick(name, pitch)?;
        let mut cue = self.bank.cue(&id, MILLI, pitch)?;
        // the noise is one short loop: start each sound somewhere new in it
        cue["noise_at"] = dec(self.rng.below(self.tuning.noise_ms as u32) as i64);
        if let Some(at) = at {
            let reach = self.bank.reach(&id)?;
            let place = self.place(at, reach);
            if falloff(distance(at, self.ear), reach) < self.tuning.audible {
                return Ok(false);
            }
            cue["place"] = place;
            if self.bank.find_cue(&id)?.get("track").is_some() {
                let key = format!("{id}:{who}:{tick}");
                let seconds = parse_milli(&cue["length"], "length")?;
                cue["key"] = json!(key);
                self.tracked.push(Tracked {
                    key,
                    who,
                    at,
                    reach,
                    ends: tick + seconds * TICKS_PER_SECOND / MILLI,
                });
            }
        }
        self.pending.push(cue);
        Ok(true)
    }

    /// Something happened: `name` is a cue id or a group such as
    /// `sheepdog_bark`, `who` tells apart two of the same in a tick, `at` is
    /// None for a sound with no place (menus, your own powerup). A rollback
    /// replays the same tick's events, so an event already played is skipped.
    pub fn event(&mut self, name: &str, who: i64, at: Option<Point>, tick: i64) -> Result<(), String> {
        let key = (tick, who, name.to_string());
        if self.seen.contains(&key) {
            return Ok(());
        }
        if self.seen.len() >= REMEMBERED {
            self.seen.remove(0);
        }
        self.seen.push(key);
        self.queue(name, who, at, MILLI, tick).map(|_| ())
    }

    /// Moves a tracked sound's source, such as the sheep wearing the bell.
    pub fn source(&mut self, who: i64, at: Point) {
        for t in self.tracked.iter_mut().filter(|t| t.who == who) {
            t.at = at;
        }
    }

    /// One seat's footsteps and panting.
    pub fn mover(
        &mut self,
        seat: usize,
        at: Point,
        gait: Gait,
        panting: bool,
        tick: i64,
    ) -> Result<(), String> {
        if seat >= SEATS {
            return Ok(());
        }
        let stride = match gait {
            Gait::Still => None,
            Gait::Run => Some(("step", self.tuning.run_step)),
            Gait::Sprint => Some(("step_fast", self.tuning.sprint_step)),
        };
        if let Some((group, ticks)) = stride {
            if tick >= self.steps[seat] {
                self.steps[seat] = tick + ticks;
                self.queue(group, seat as i64, Some(at), MILLI, tick)?;
            }
        }
        if panting && tick >= self.pants[seat] {
            self.pants[seat] = tick + self.tuning.pant;
            self.queue("pant", seat as i64, Some(at), MILLI, tick)?;
        }
        Ok(())
    }

    /// The flock this tick. Baas start one at a time, at a rate that does not
    /// grow with the flock, and only while fewer than `max_baas` are sounding;
    /// each is the loudest of a few sheep picked at random, so near sheep speak
    /// most and far ones now and then. Hooves are one sound whose level follows
    /// how many sheep are moving nearby.
    pub fn flock(&mut self, sheep: &[HeardSheep], tick: i64) -> Result<(), String> {
        let t = self.tuning;
        self.baas.retain(|end| *end > tick);
        if sheep.is_empty() {
            self.hooves = None;
            return Ok(());
        }
        if tick >= self.next_baa {
            // a small flock speaks less often
            let thin = (t.baa_full_flock / sheep.len() as i64).max(1);
            let wait = t.baa_every + self.rng.below(t.baa_spread as u32 + 1) as i64;
            self.next_baa = tick + wait * thin;
            if self.baas.len() < t.max_baas {
                self.baa(sheep, tick)?;
            }
        }
        if tick % t.hooves_every == 0 {
            self.hoof_count(sheep)?;
        }
        Ok(())
    }

    fn baa(&mut self, sheep: &[HeardSheep], tick: i64) -> Result<(), String> {
        let t = self.tuning;
        let reach = self.bank.reach("sheep_baa_1")?;
        let mut best: Option<(i64, usize)> = None;
        for _ in 0..t.baa_tries {
            let k = self.rng.below(sheep.len() as u32) as usize;
            let level = falloff(distance(sheep[k].at, self.ear), reach);
            if best.is_none_or(|(top, _)| level > top) {
                best = Some((level, k));
            }
        }
        let Some((_, k)) = best else {
            return Ok(());
        };
        let lamb = sheep[k].lamb || self.rng.below(100) < t.lamb_share;
        let group = if lamb { "lamb_baa" } else { "sheep_baa" };
        // each sheep keeps its own voice: a pitch from its place in the flock
        let voice = MILLI - 120 + (k as i64 * 37) % 240;
        if self.queue(group, k as i64, Some(sheep[k].at), voice, tick)? {
            if let Some(cue) = self.pending.last() {
                let length = parse_milli(&cue["length"], "length")?;
                self.baas.push(tick + length * TICKS_PER_SECOND / MILLI);
            }
        }
        Ok(())
    }

    fn hoof_count(&mut self, sheep: &[HeardSheep]) -> Result<(), String> {
        let t = self.tuning;
        let reach = self.bank.reach("hooves")?;
        let (mut count, mut level, mut sx, mut sy, mut sz) = (0i64, 0i64, 0i64, 0i64, 0i64);
        for s in sheep.iter().filter(|s| s.moving) {
            let d = distance(s.at, self.ear);
            if d < reach.far {
                let g = falloff(d, reach);
                count += 1;
                level += g;
                sx += s.at.x as i64 * g / MILLI;
                sy += s.at.y as i64 * g / MILLI;
                sz += s.at.z as i64 * g / MILLI;
            }
        }
        if count == 0 || level == 0 {
            self.hooves = None;
            return Ok(());
        }
        let center = Point {
            x: (sx * MILLI / level) as Fx,
            y: (sy * MILLI / level) as Fx,
            z: (sz * MILLI / level) as Fx,
        };
        // many sheep moving nearby at the same time make it full
        let gain = mul((level / t.hooves_full).min(MILLI), t.hooves_gain);
        self.hooves = Some((gain, center));
        Ok(())
    }

    /// Everything the page must do this frame.
    pub fn frame(&mut self, tick: i64) -> Result<Value, String> {
        self.tracked.retain(|t| t.ends > tick);
        let moves: Vec<Value> = self
            .tracked
            .iter()
            .map(|t| json!({"key": t.key, "place": self.place(t.at, t.reach)}))
            .collect();
        let mut loops = Vec::new();
        if let Some((gain, at)) = self.hooves {
            let reach = self.bank.reach("hooves")?;
            let mut place = self.place(at, reach);
            place["gain"] = dec(gain);
            loops.push(json!({"id": "hooves", "place": place}));
        }
        let cues = std::mem::take(&mut self.pending);
        Ok(json!({"cues": cues, "moves": moves, "loops": loops}))
    }
}

// ---- the page's door ----

fn answer(result: Result<Value, String>) -> String {
    match result {
        Ok(v) => json!({ "ok": v }).to_string(),
        Err(e) => json!({ "error": e }).to_string(),
    }
}

/// What a page needs to make sound, and the sound test page's pretend flock.
/// The shell's game holds a `Mixer` of its own and uses the same calls.
#[derive(Debug, Clone)]
pub struct Desk {
    bank: Bank,
    mixer: Mixer,
    tick: i64,
}

impl Desk {
    pub fn new(seed: u64) -> Result<Desk, String> {
        let bank = bank()?;
        let mixer = Mixer::new(bank.clone(), seed)?;
        Ok(Desk { bank, mixer, tick: 0 })
    }

    /// Everything `startAudio` takes, besides the noise.
    pub fn setup(&self) -> String {
        answer(self.bank.setup())
    }

    /// Every sound and tune, for the sound test page.
    pub fn listing(&self) -> String {
        answer(self.bank.listing())
    }

    pub fn cue(&self, id: &str) -> String {
        answer(self.bank.preview(id))
    }

    pub fn tune(&self, id: &str) -> String {
        answer(self.bank.tune(id))
    }

    pub fn noise(&self) -> Vec<u8> {
        self.bank.noise_wav().unwrap_or_default()
    }

    pub fn master(&self, percent: i64) -> String {
        answer(Ok(dec(master_gain(percent))))
    }

    /// The main gain in a match (`play_gain`).
    pub fn play_master(&self, percent: i64) -> String {
        answer(self.bank.loudest_in_play().map(|l| dec(play_gain(percent, l))))
    }

    /// One frame of `count` sheep moving around the listener, in a ring out to
    /// the far side of hearing.
    pub fn flock_frame(&mut self, count: i64) -> String {
        answer(self.next_flock_frame(count))
    }

    fn next_flock_frame(&mut self, count: i64) -> Result<Value, String> {
        self.tick += 1;
        let sheep: Vec<HeardSheep> = (0..count).map(|k| pretend_sheep(k, count, self.tick)).collect();
        self.mixer.flock(&sheep, self.tick)?;
        self.mixer.frame(self.tick)
    }
}

/// A sheep of the test page's flock: spread on a slowly turning spiral.
fn pretend_sheep(k: i64, count: i64, tick: i64) -> HeardSheep {
    let angle = (k * fx::TURN as i64 / 7 + tick) as i32;
    let radius = fx::from_int(3) + fx::from_int(60) / count.max(1) as i32 * k as i32;
    HeardSheep {
        at: Point {
            x: fx::mul(fx::cos(angle), radius),
            y: 0,
            z: fx::mul(fx::sin(angle), radius),
        },
        lamb: k % 5 == 0,
        moving: k % 3 == 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bank_ok() -> Bank {
        bank().expect("the file reads")
    }

    #[test]
    fn decimals_read_as_thousandths_and_write_back() {
        assert_eq!(parse_milli(&json!(440), "f"), Ok(440_000));
        let quarter: Value = serde_json::from_str("0.25").unwrap();
        assert_eq!(parse_milli(&quarter, "g"), Ok(250));
        assert_eq!(dec(250).to_string(), "0.25");
        assert_eq!(dec(-1500).to_string(), "-1.5");
        assert!(parse_milli(&json!("loud"), "g").unwrap_err().starts_with("g "));
    }

    #[test]
    fn the_chanter_is_the_nine_note_just_scale_over_low_a() {
        // worked by hand from low A = 476 Hz and the ratios in Macpherson's
        // table, with high A set a little under the octave
        let b = bank_ok();
        let hz = |p| b.pitch_hz(p).unwrap();
        assert_eq!(hz(Pitch::LowA), 476_000);
        assert_eq!(hz(Pitch::LowG), 416_500); // 7:8
        assert_eq!(hz(Pitch::B), 535_500); // 9:8
        assert_eq!(hz(Pitch::C), 595_000); // 5:4
        assert_eq!(hz(Pitch::D), 634_666); // 4:3
        assert_eq!(hz(Pitch::E), 714_000); // 3:2
        assert_eq!(hz(Pitch::F), 793_333); // 5:3
        assert_eq!(hz(Pitch::HighG), 833_000); // 7:4
        assert_eq!(hz(Pitch::HighA), 947_716); // 1991:1000
    }

    fn at(point: &Value) -> i64 {
        parse_milli(&point["t"], "t").unwrap()
    }

    #[test]
    fn the_drones_sound_under_every_chanter_note() {
        let b = bank_ok();
        for id in ["scotland_the_brave", "flowers_of_the_forest"] {
            let tune = b.tune(id).unwrap();
            let layers = tune["layers"].as_array().unwrap();
            let chanter = layers.last().unwrap();
            let first = at(&chanter["freq"][0]);
            let last = at(chanter["gain"].as_array().unwrap().last().unwrap());
            assert_eq!(layers.len(), 4, "two tenors, a bass and the chanter");
            for drone in &layers[..3] {
                let gain = drone["gain"].as_array().unwrap();
                assert!(at(&gain[0]) < first, "{id}: a drone starts after the chanter");
                assert!(
                    at(gain.last().unwrap()) >= last,
                    "{id}: a drone stops before the chanter"
                );
            }
        }
    }

    #[test]
    fn the_drones_sit_one_and_two_octaves_under_low_a() {
        let tune = bank_ok().tune("scotland_the_brave").unwrap();
        let hz = |k: usize| parse_milli(&tune["layers"][k]["freq"][0]["v"], "v").unwrap();
        assert_eq!(hz(0), 238_000);
        assert_eq!(hz(1), 238_000);
        assert_eq!(hz(2), 119_000);
    }

    #[test]
    fn the_chanter_strikes_in_on_e_after_the_drones() {
        let b = bank_ok();
        let score = b.score("flowers_of_the_forest").unwrap();
        assert_eq!(score["steps"][0]["note"], "e");
        assert!(parse_milli(&score["steps"][0]["t"], "t").unwrap() > 0);
    }

    #[test]
    fn an_unknown_cue_or_tune_is_refused_by_name() {
        let b = bank_ok();
        assert!(b.tune("greensleeves").unwrap_err().contains("greensleeves"));
        assert!(b.cue("kazoo", MILLI, MILLI).unwrap_err().contains("kazoo"));
    }

    // ---- hearing ----

    const METER: Fx = fx::ONE;

    fn point(x: i32, z: i32) -> Point {
        Point {
            x: fx::from_int(x),
            y: 0,
            z: fx::from_int(z),
        }
    }

    fn mixer() -> Mixer {
        Mixer::new(bank_ok(), 11).unwrap()
    }

    fn milli_at(v: &Value) -> i64 {
        parse_milli(v, "v").unwrap()
    }

    /// Sheep on a ring of this radius around the listener.
    fn ring(count: i32, radius: i32) -> Vec<HeardSheep> {
        (0..count)
            .map(|k| {
                let angle = k * fx::TURN / count;
                HeardSheep {
                    at: Point {
                        x: fx::mul(fx::cos(angle), fx::from_int(radius)),
                        y: 0,
                        z: fx::mul(fx::sin(angle), fx::from_int(radius)),
                    },
                    lamb: false,
                    moving: true,
                }
            })
            .collect()
    }

    /// The cues a flock starts over `ticks`.
    fn run_flock(sheep: &[HeardSheep], ticks: i64) -> Vec<Value> {
        let mut m = mixer();
        let mut all = Vec::new();
        for tick in 0..ticks {
            m.flock(sheep, tick).unwrap();
            all.extend(m.frame(tick).unwrap()["cues"].as_array().unwrap().clone());
        }
        all
    }

    #[test]
    fn a_sound_is_full_when_near_then_falls_with_distance() {
        let reach = Reach {
            near: 6000,
            far: 70_000,
            floor: 0,
            far_hz: 1_800_000,
        };
        assert_eq!(falloff(3000, reach), 1000);
        assert_eq!(falloff(6000, reach), 1000);
        // 6/12 is 500; the taper is 58/64 = 906; 500 * 906 is 453
        assert_eq!(falloff(12_000, reach), 453);
        assert_eq!(falloff(70_000, reach), 0);
        assert_eq!(falloff(90_000, reach), 0);
    }

    #[test]
    fn a_floored_sound_carries_to_the_far_end() {
        let reach = Reach {
            near: 30_000,
            far: 300_000,
            floor: 300,
            far_hz: 1_800_000,
        };
        assert_eq!(falloff(290_000, reach), 300);
        assert_eq!(falloff(900_000, reach), 300);
        // 30/60 is 500; the taper is 240/270 = 888; 500 * 888 is 444
        assert_eq!(falloff(60_000, reach), 444);
    }

    #[test]
    fn distance_holds_past_the_length_q16_squares_overflow_at() {
        assert_eq!(distance(point(0, 0), point(300, 400)), 500_000);
    }

    #[test]
    fn the_main_volume_follows_a_square() {
        assert_eq!(master_gain(0), 0);
        assert_eq!(master_gain(50), 250);
        assert_eq!(master_gain(100), 1000);
        assert_eq!(master_gain(160), 1000);
    }

    #[test]
    fn a_match_at_full_volume_plays_at_six_tenths_of_the_test_page() {
        let (full, half) = (600, 150);
        assert_eq!(play_gain(100, 600), full);
        assert_eq!(play_gain(50, 600), half);
        assert_eq!(play_gain(0, 600), 0);
        assert_eq!(play_gain(160, 600), full);
        assert_eq!(master_gain(100), 1000, "the test page keeps its level");
        assert_eq!(bank_ok().loudest_in_play(), Ok(600), "data/sound.json");
    }

    #[test]
    fn each_team_wins_to_its_own_tune() {
        let b = bank_ok();
        assert_eq!(win_tune(&b, true).unwrap(), "scotland_the_brave");
        assert_eq!(win_tune(&b, false).unwrap(), "flowers_of_the_forest");
    }

    fn placed(m: &Mixer, at: Point) -> [i64; 3] {
        let p = m.place(
            at,
            Reach {
                near: 1000,
                far: 100_000,
                floor: 0,
                far_hz: 1_800_000,
            },
        );
        let pos = |k: usize| milli_at(&p["pos"][k]);
        [pos(0), pos(1), pos(2)]
    }

    #[test]
    fn direction_is_relative_to_where_the_listener_faces() {
        let mut m = mixer();
        // facing -z, as the page's listener does: +x is to the right
        m.listen(point(0, 0), 0, -METER);
        assert_eq!(placed(&m, point(10, 0)), [10_000, 0, 0]);
        assert_eq!(placed(&m, point(0, -10)), [0, 0, -10_000]);
        assert_eq!(placed(&m, point(0, 10)), [0, 0, 10_000]);
        // turned to face +x, the sound on +x is dead ahead and -z is on the left
        m.listen(point(0, 0), METER, 0);
        assert_eq!(placed(&m, point(10, 0)), [0, 0, -10_000]);
        assert_eq!(placed(&m, point(0, -10)), [-10_000, 0, 0]);
        // and the listener's own place counts
        m.listen(point(100, 0), 0, -METER);
        assert_eq!(placed(&m, point(110, 0)), [10_000, 0, 0]);
    }

    #[test]
    fn a_sound_above_the_listener_is_placed_above() {
        let m = mixer();
        let at = Point {
            y: 4 * METER,
            ..Point::default()
        };
        assert_eq!(placed(&m, at), [0, 4000, 0]);
    }

    #[test]
    fn a_far_sound_is_quieter_and_duller() {
        let m = mixer();
        let reach = m.bank.reach("sheepdog_bark_1").unwrap();
        let near = m.place(point(5, 0), reach);
        let far = m.place(point(150, 0), reach);
        assert!(milli_at(&far["gain"]) < milli_at(&near["gain"]));
        assert!(milli_at(&far["muffle"]) < milli_at(&near["muffle"]));
    }

    #[test]
    fn an_event_a_rollback_replays_plays_once() {
        let mut m = mixer();
        m.event("sheepdog_bark", 2, Some(point(1, 1)), 300).unwrap();
        m.event("sheepdog_bark", 2, Some(point(1, 1)), 300).unwrap();
        assert_eq!(m.frame(300).unwrap()["cues"].as_array().unwrap().len(), 1);
        m.event("sheepdog_bark", 2, Some(point(1, 1)), 330).unwrap();
        m.event("sheepdog_bark", 3, Some(point(1, 1)), 330).unwrap();
        assert_eq!(m.frame(330).unwrap()["cues"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn a_placed_cue_carries_its_place_and_a_flat_one_does_not() {
        let mut m = mixer();
        m.event("click", 0, None, 1).unwrap();
        m.event("wolf_eat", 0, Some(point(3, 0)), 1).unwrap();
        let frame = m.frame(1).unwrap();
        assert!(frame["cues"][0].get("place").is_none());
        assert!(frame["cues"][1].get("place").is_some());
    }

    #[test]
    fn four_hundred_sheep_never_pass_the_voice_budget() {
        let mut m = mixer();
        // far faster than the file asks, so the budget is what holds them back
        m.tuning.baa_every = 1;
        m.tuning.baa_spread = 1;
        let sheep = ring(400, 8);
        let mut most = 0;
        for tick in 0..600 {
            m.flock(&sheep, tick).unwrap();
            most = most.max(m.baas.len());
            m.frame(tick).unwrap();
        }
        assert_eq!(most, m.tuning.max_baas);
    }

    #[test]
    fn a_big_flock_baas_no_more_often_than_a_middling_one() {
        let hundred = run_flock(&ring(100, 8), 3600);
        let four = run_flock(&ring(400, 8), 3600);
        assert!(
            four.len() * 10 <= hundred.len() * 12,
            "{} against {}",
            four.len(),
            hundred.len()
        );
        assert!(four.len() >= 60, "a minute of 400 sheep gave {} baas", four.len());
    }

    #[test]
    fn a_small_flock_speaks_less_often() {
        let ten = run_flock(&ring(10, 8), 3600);
        let hundred = run_flock(&ring(100, 8), 3600);
        assert!(
            ten.len() * 2 < hundred.len(),
            "{} against {}",
            ten.len(),
            hundred.len()
        );
    }

    #[test]
    fn near_sheep_speak_more_than_far_ones() {
        let mut sheep = ring(200, 7);
        sheep.extend(ring(200, 45));
        let cues = run_flock(&sheep, 3600);
        let near = cues
            .iter()
            .filter(|c| milli_at(&c["place"]["gain"]) > 800)
            .count();
        assert!(near * 10 > cues.len() * 8, "{near} of {}", cues.len());
        assert!(near < cues.len(), "far sheep should speak now and then");
    }

    #[test]
    fn a_flock_out_of_earshot_is_silent() {
        let cues = run_flock(&ring(100, 200), 600);
        assert!(cues.is_empty());
    }

    #[test]
    fn hooves_follow_the_moving_sheep_and_stop_when_they_stop() {
        let mut m = mixer();
        let mut sheep = ring(40, 8);
        m.flock(&sheep, 0).unwrap();
        let gain = |m: &mut Mixer| milli_at(&m.frame(0).unwrap()["loops"][0]["place"]["gain"]);
        let full = gain(&mut m);
        assert!(full > 0);
        sheep.iter_mut().skip(5).for_each(|s| s.moving = false);
        m.flock(&sheep, 6).unwrap();
        assert!(gain(&mut m) < full);
        sheep.iter_mut().for_each(|s| s.moving = false);
        m.flock(&sheep, 12).unwrap();
        assert!(m.frame(12).unwrap()["loops"].as_array().unwrap().is_empty());
    }

    #[test]
    fn hooves_sound_from_the_side_the_sheep_are_on() {
        let mut m = mixer();
        let sheep: Vec<HeardSheep> = (0..20)
            .map(|k| HeardSheep {
                at: point(12, k - 10),
                lamb: false,
                moving: true,
            })
            .collect();
        m.flock(&sheep, 0).unwrap();
        assert!(milli_at(&m.frame(0).unwrap()["loops"][0]["place"]["pos"][0]) > 9000);
    }

    #[test]
    fn steps_come_faster_at_a_sprint_and_stop_when_still() {
        let count = |gait| {
            let mut m = mixer();
            (0..600)
                .map(|tick| {
                    m.mover(1, point(1, 0), gait, false, tick).unwrap();
                    m.frame(tick).unwrap()["cues"].as_array().unwrap().len()
                })
                .sum::<usize>()
        };
        assert_eq!(count(Gait::Still), 0);
        assert_eq!(count(Gait::Run), 30);
        assert_eq!(count(Gait::Sprint), 43);
    }

    #[test]
    fn panting_keeps_its_own_beat() {
        let mut m = mixer();
        let mut pants = 0;
        for tick in 0..420 {
            m.mover(0, point(0, 0), Gait::Still, true, tick).unwrap();
            pants += m.frame(tick).unwrap()["cues"].as_array().unwrap().len();
        }
        assert_eq!(pants, 10);
    }

    #[test]
    fn the_bell_rings_once_for_about_ten_seconds() {
        let cue = bank_ok().cue("sheep_bell", MILLI, MILLI).unwrap();
        let seconds = milli_at(&cue["length"]);
        assert!((9_500..12_000).contains(&seconds), "{seconds}");
        // one voice per partial, however many strikes
        assert!(cue["layers"].as_array().unwrap().len() <= 6);
        let strikes = cue["layers"][0]["gain"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["m"] == "setValueAtTime")
            .count();
        assert!(strikes > 15, "{strikes}");
    }

    #[test]
    fn the_bell_carries_across_the_whole_map_and_follows_its_sheep() {
        let mut m = mixer();
        let far = point(255, 135);
        m.event("sheep_bell", 77, Some(far), 100).unwrap();
        let frame = m.frame(100).unwrap();
        assert!(milli_at(&frame["cues"][0]["place"]["gain"]) >= 300);
        assert!(frame["moves"].as_array().unwrap().len() == 1);
        m.source(77, point(-40, 0));
        assert_eq!(
            milli_at(&m.frame(101).unwrap()["moves"][0]["place"]["pos"][0]),
            -40_000
        );
    }

    #[test]
    fn a_tracked_sound_stops_being_followed_when_it_ends() {
        let mut m = mixer();
        m.event("sheep_bell", 77, Some(point(10, 0)), 0).unwrap();
        m.frame(0).unwrap();
        let ticks = milli_at(&bank_ok().cue("sheep_bell", MILLI, MILLI).unwrap()["length"]) * 60 / MILLI;
        assert_eq!(m.frame(ticks - 1).unwrap()["moves"].as_array().unwrap().len(), 1);
        assert!(m.frame(ticks).unwrap()["moves"].as_array().unwrap().is_empty());
    }

    #[test]
    fn the_stampede_is_a_snarl_then_a_rush_of_hooves() {
        let cue = bank_ok().cue("stampede", MILLI, MILLI).unwrap();
        let start = |k: usize| milli_at(&cue["layers"][k]["start"]);
        assert_eq!(cue["layers"][0]["wave"], "sawtooth");
        assert_eq!(cue["layers"][2]["wave"], "noise");
        assert!(start(2) > start(0) + 400);
    }

    #[test]
    fn every_sound_in_the_file_compiles_and_every_placed_one_has_a_reach() {
        let b = bank_ok();
        for c in b.file["cues"].as_array().unwrap() {
            let id = c["id"].as_str().unwrap();
            if c.get("loop").is_none() {
                b.cue(id, MILLI, MILLI).unwrap();
            }
            if c.get("reach").is_some() {
                b.reach(id).unwrap();
            }
        }
        b.dump().unwrap();
    }

    #[test]
    fn the_listing_names_every_sound_and_tune() {
        let b = bank_ok();
        let page = b.listing().unwrap();
        let named = page["cues"].as_array().unwrap().len() + page["loops"].as_array().unwrap().len();
        assert_eq!(named, b.file["cues"].as_array().unwrap().len());
        assert_eq!(page["tunes"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn a_group_picks_among_its_variants() {
        let mut m = mixer();
        let picked: Vec<String> = (0..60).map(|_| m.pick("sheep_baa", MILLI).unwrap().0).collect();
        for k in 1..=4 {
            assert!(picked.contains(&format!("sheep_baa_{k}")), "{k}");
        }
    }

    #[test]
    fn the_bell_stays_bright_enough_to_place_from_the_far_end() {
        let m = mixer();
        let reach = m.bank.reach("sheep_bell").unwrap();
        let far = milli_at(&m.place(point(255, 135), reach)["muffle"]);
        // its top partial is 5388 Hz
        assert!(far >= 5_400_000, "{far}");
    }

    #[test]
    fn repeated_sounds_start_their_noise_in_different_places() {
        let mut m = mixer();
        let mut starts = Vec::new();
        for tick in 0..20 {
            m.event("dig", 0, None, tick).unwrap();
            starts.push(milli_at(&m.frame(tick).unwrap()["cues"][0]["noise_at"]));
        }
        starts.sort();
        starts.dedup();
        assert!(starts.len() > 10, "{starts:?}");
    }

    #[test]
    fn the_limiter_leaves_the_mix_alone_until_near_full_scale() {
        let limiter = bank_ok().setup().unwrap()["limiter"].clone();
        assert!(milli_at(&limiter["threshold"]) >= -3000);
        assert!(milli_at(&limiter["ratio"]) >= 12_000);
        assert_eq!(milli_at(&limiter["knee"]), 0);
    }

    #[test]
    fn a_loop_plays_for_a_few_seconds_on_the_test_page() {
        let b = bank_ok();
        assert_eq!(milli_at(&b.preview("hooves").unwrap()["length"]), 4000);
        assert_eq!(b.preview("click").unwrap(), b.cue("click", MILLI, MILLI).unwrap());
    }

    #[test]
    fn the_noise_is_a_wav_file_and_the_same_each_time() {
        let b = bank_ok();
        let wav = b.noise_wav().unwrap();
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(wav.len(), 44 + 2 * 44_100);
        assert_eq!(wav, b.noise_wav().unwrap());
        assert!(wav[44..].iter().any(|byte| *byte > 200));
    }
}
