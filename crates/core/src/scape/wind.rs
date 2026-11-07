//! The wind: four summed sines from `data/wind.json`, the same in the grass
//! and ground shaders, so sway and gusts line up. Its phases come from the
//! tick and live nowhere: rollback has nothing to restore.

use serde::Deserialize;

use crate::fx::{self, Fx, ONE, TURN};

use super::cm;

pub const WIND_JSON: &str = include_str!("../../../../data/wind.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
struct WaveFile {
    wavelength_cm: i32,
    speed_cm: i32,
    share_pct: i32,
    turn_degrees: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct WindFile {
    from_degrees: i32,
    bend_cm: i32,
    bend_full_cm: i32,
    waves: [WaveFile; 4],
}

/// What the shaders read, as four-wide values with no indexing: a wave's
/// heading times its number of radians a meter, its phase, its share.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wind {
    /// Which way it blows, on the ground.
    pub dir: [Fx; 2],
    /// How far a full blade leans at full gust.
    pub bend: Fx,
    /// Radians a meter along x and z.
    pub kx: [Fx; 4],
    pub kz: [Fx; 4],
    /// Radians a tick.
    step: [i64; 4],
    pub share: [Fx; 4],
}

/// Fixed-point radians a full turn holds: 2π.
const TAU: i64 = 411_775;

fn heading(degrees: i32) -> [Fx; 2] {
    let a = degrees * TURN / 360;
    [fx::cos(a), fx::sin(a)]
}

impl Wind {
    pub fn parse(text: &str) -> Result<Wind, String> {
        let file: WindFile = serde_json::from_str(text).map_err(|e| format!("The wind file: {e}"))?;
        let mut wind = Wind {
            dir: heading(file.from_degrees + 180),
            bend: cm(file.bend_cm),
            kx: [0; 4],
            kz: [0; 4],
            step: [0; 4],
            share: [0; 4],
        };
        for (i, w) in file.waves.iter().enumerate() {
            let way = heading(file.from_degrees + 180 + w.turn_degrees);
            // 2π / wavelength in meters
            let k = (TAU * 100 / w.wavelength_cm as i64) as Fx;
            wind.kx[i] = fx::mul(way[0], k);
            wind.kz[i] = fx::mul(way[1], k);
            // radians a tick: speed in meters a second, times k, over 60
            wind.step[i] = k as i64 * w.speed_cm as i64 / (100 * crate::scape::TICKS);
            wind.share[i] = w.share_pct * ONE / 100;
        }
        let _ = file.bend_full_cm;
        Ok(wind)
    }

    /// The file this game ships with.
    pub fn shipped() -> Wind {
        match Wind::parse(WIND_JSON) {
            Ok(w) => w,
            Err(_) => Wind {
                dir: [ONE, 0],
                bend: 0,
                kx: [0; 4],
                kz: [0; 4],
                step: [0; 4],
                share: [0; 4],
            },
        }
    }

    /// Each wave's phase at this tick, wrapped to under a full circle.
    /// `wrapping_mul` keeps it right past the end of an `i64`, and two
    /// browsers at the same tick agree to the bit.
    pub fn phases(&self, tick: u32) -> [Fx; 4] {
        self.step
            .map(|s| (s.wrapping_mul(tick as i64).rem_euclid(TAU)) as Fx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = r#"{"from_degrees": 180, "bend_cm": 6, "bend_full_cm": 20, "waves": [
      {"wavelength_cm": 1000, "speed_cm": 100, "share_pct": 50, "turn_degrees": 0},
      {"wavelength_cm": 500, "speed_cm": 200, "share_pct": 30, "turn_degrees": 90},
      {"wavelength_cm": 1000, "speed_cm": 100, "share_pct": 10, "turn_degrees": 0},
      {"wavelength_cm": 1000, "speed_cm": 100, "share_pct": 10, "turn_degrees": 0}]}"#;

    #[test]
    fn the_wind_blows_away_from_where_it_comes_from() {
        let w = Wind::parse(FILE).unwrap();
        assert!(
            (w.dir[0] - ONE).abs() < ONE / 200 && w.dir[1].abs() < ONE / 200,
            "{:?}",
            w.dir
        );
    }

    #[test]
    fn a_wave_has_the_number_of_radians_a_meter_its_wavelength_gives() {
        let w = Wind::parse(FILE).unwrap();
        // 10 m: 2π/10 = 0.6283 radians a meter, along +x for the first wave
        assert!((w.kx[0] - 41177).abs() < 150, "{}", w.kx[0]);
        // the second turns a quarter, so it runs along z, twice as tight
        assert!((w.kz[1] - 82355).abs() < 400, "{}", w.kz[1]);
        assert!(w.kx[1].abs() < 400, "{}", w.kx[1]);
    }

    #[test]
    fn phases_advance_with_the_tick_and_wrap_under_a_circle() {
        let w = Wind::parse(FILE).unwrap();
        assert_eq!(w.phases(0), [0; 4]);
        // 1 m/s at 2π/10 radians a meter, one second on: 0.6283 radians
        let p = w.phases(60);
        assert!((p[0] - 41177).abs() < 80, "{}", p[0]);
        for tick in [1_000_000u32, u32::MAX] {
            assert!(w.phases(tick).iter().all(|p| (0..TAU as Fx).contains(p)));
        }
    }

    #[test]
    fn the_shipped_file_reads_and_its_shares_add_up() {
        let w = Wind::parse(WIND_JSON).unwrap();
        // 42 + 26 + 20 + 12 percent, truncated channel by channel
        assert!((w.share.iter().sum::<Fx>() - ONE).abs() <= 4);
    }

    #[test]
    fn a_broken_file_names_itself() {
        assert!(Wind::parse("{}").unwrap_err().starts_with("The wind file"));
    }
}
