//! The sky as a pure function of one phase, 0 for full day and `ONE` for
//! full night, so rollback has nothing to keep: the core hands it the Night's
//! darkness and the same phase is the same sky on every screen.
//!
//! The sun and moon each ride their own arc, the sun sinking out of the sky
//! while the moon climbs opposite it, and the light swings over to the moon
//! at the moment the sun's gone, when there is almost none of it to jump.

use crate::fx::{self, Fx, ONE};
use crate::mesh::{blend, rgb, Rgb};
use crate::render::geom::{deg, facing, lerp};
use crate::render::Sky;

use super::cm;
use super::noise::band;

/// Each stop on the way down: its phase, then what it sets.
struct Stop {
    at: Fx,
    fill: Rgb,
    ground: Rgb,
    horizon: Rgb,
    zenith: Rgb,
    fog: (Fx, Fx),
    shade: Fx,
}

const DAY: Stop = Stop {
    at: 0,
    fill: rgb(176, 188, 204),
    ground: rgb(118, 128, 96),
    horizon: rgb(205, 222, 236),
    zenith: rgb(82, 140, 196),
    fog: (fx::from_int(200), fx::from_int(2100)),
    shade: cm(38),
};
const SUNSET: Stop = Stop {
    at: cm(58),
    fill: rgb(116, 104, 124),
    ground: rgb(72, 64, 62),
    horizon: rgb(240, 158, 100),
    zenith: rgb(58, 80, 136),
    fog: (fx::from_int(130), fx::from_int(2000)),
    shade: cm(40),
};
const BLUE: Stop = Stop {
    at: cm(76),
    fill: rgb(62, 72, 108),
    ground: rgb(34, 38, 56),
    horizon: rgb(62, 76, 122),
    zenith: rgb(18, 26, 62),
    fog: (fx::from_int(60), fx::from_int(230)),
    shade: cm(34),
};
const NIGHT: Stop = Stop {
    at: ONE,
    fill: rgb(50, 62, 98),
    ground: rgb(22, 28, 44),
    horizon: rgb(30, 40, 70),
    zenith: rgb(9, 14, 34),
    fog: (fx::from_int(50), fx::from_int(190)),
    shade: cm(30),
};
const STOPS: [Stop; 4] = [DAY, SUNSET, BLUE, NIGHT];

const SUN_HEADING: i32 = 215;
const MOON_HEADING: i32 = 35;
const SUN_DAY: i32 = 42;
const SUN_NIGHT: i32 = -30;
const MOON_DAY: i32 = -42;
const MOON_NIGHT: i32 = 46;

/// Where the sun goes below the true horizon.
const SUN_GONE: Fx = ONE * SUN_DAY / (SUN_DAY - SUN_NIGHT);

const SUN_LIGHT: Rgb = rgb(118, 110, 92);
const SUNSET_LIGHT: Rgb = rgb(150, 92, 52);
const MOON_LIGHT: Rgb = rgb(66, 82, 128);
const SUN_DISC: Rgb = rgb(255, 248, 226);
const SUNSET_DISC: Rgb = rgb(255, 150, 72);
const MOON_DISC: Rgb = rgb(236, 240, 252);

fn scaled(c: Rgb, t: Fx) -> Rgb {
    c.map(|v| fx::mul(v, t))
}

/// `sky` with its light and its colors at this share, for a viewer's night.
/// The fog stays where it is and the moon and stars stay bright.
pub fn dimmed(sky: Sky, by: Fx) -> Sky {
    Sky {
        sun: scaled(sky.sun, by),
        fill: scaled(sky.fill, by),
        ground: scaled(sky.ground, by),
        horizon: scaled(sky.horizon, by),
        zenith: scaled(sky.zenith, by),
        ..sky
    }
}

/// The sky at `phase`.
pub fn at(phase: Fx) -> Sky {
    let t = phase.clamp(0, ONE);
    let sun_up = lerp(deg(SUN_DAY), deg(SUN_NIGHT), t);
    let moon_up = lerp(deg(MOON_DAY), deg(MOON_NIGHT), t);
    let sun_dir = facing(deg(SUN_HEADING), sun_up);
    let moon_dir = facing(deg(MOON_HEADING), moon_up);
    let i = STOPS
        .iter()
        .rposition(|s| s.at <= t)
        .unwrap_or(0)
        .min(STOPS.len() - 2);
    let (a, b) = (&STOPS[i], &STOPS[i + 1]);
    let k = fx::div(t - a.at, b.at - a.at);
    let c = |x: Rgb, y: Rgb| blend(x, y, k);
    let f = |x: Fx, y: Fx| lerp(x, y, k);
    let low = band(0, cm(40), SUN_GONE - t);
    let (light_dir, light, disc) = if t < SUN_GONE {
        let gold = blend(SUNSET_LIGHT, SUN_LIGHT, band(0, cm(45), SUN_GONE - t));
        let fading = band(0, cm(12), SUN_GONE - t);
        (sun_dir, scaled(gold, fading), blend(SUNSET_DISC, SUN_DISC, low))
    } else {
        // from here the key light and its disc are the moon's
        let rising = band(0, ONE - SUN_GONE, t - SUN_GONE);
        (moon_dir, scaled(MOON_LIGHT, rising), MOON_DISC)
    };
    Sky {
        sun_dir: light_dir,
        sun: light,
        fill: c(a.fill, b.fill),
        ground: c(a.ground, b.ground),
        horizon: c(a.horizon, b.horizon),
        zenith: c(a.zenith, b.zenith),
        sun_color: disc,
        moon_dir,
        moon_color: MOON_DISC,
        fog: (f(a.fog.0, b.fog.0), f(a.fog.1, b.fog.1)),
        shade: f(a.shade, b.shade),
        stars: band(cm(62), cm(92), t),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_phase_is_the_same_sky() {
        for p in [0, cm(13), cm(58), cm(99), ONE] {
            assert_eq!(at(p), at(p));
        }
    }

    #[test]
    fn the_sun_sinks_as_the_moon_rises_through_the_change() {
        let mut moon = -ONE;
        for step in 1..=20 {
            let sky = at(step * ONE / 20);
            assert!(sky.moon_dir.y > moon);
            moon = sky.moon_dir.y;
        }
        assert!(at(0).sun_dir.y > cm(60));
        assert!(at(0).moon_dir.y < 0);
        assert!(at(ONE).moon_dir.y > cm(60));
    }

    #[test]
    fn the_moon_is_up_before_the_sun_is_gone() {
        let sky = at(SUN_GONE - cm(2));
        assert!(
            sky.moon_dir.y > 0 && sky.sun_dir.y > 0 && sky.sun_dir.y < cm(10),
            "{:?} {:?}",
            sky.moon_dir,
            sky.sun_dir
        );
    }

    #[test]
    fn the_light_never_jumps_on_the_way_down() {
        let mut last = at(0);
        for step in 1..=300 {
            let sky = at(step * ONE / 300);
            for ch in 0..3 {
                assert!((sky.sun[ch] - last.sun[ch]).abs() < cm(3), "step {step}");
                assert!((sky.fill[ch] - last.fill[ch]).abs() < cm(3), "step {step}");
                assert!((sky.horizon[ch] - last.horizon[ch]).abs() < cm(3), "step {step}");
            }
            last = sky;
        }
    }

    #[test]
    fn a_dimmed_sky_is_darker_with_the_fog_where_it_was() {
        let (n, d) = (at(ONE), dimmed(at(ONE), ONE / 2));
        assert_eq!(d.fill[2], n.fill[2] / 2);
        assert_eq!(d.horizon[2], n.horizon[2] / 2);
        assert_eq!(d.fog, n.fog);
        assert_eq!(d.stars, ONE);
    }

    #[test]
    fn night_is_darker_and_bluer_than_day_and_still_lit() {
        let (d, n) = (at(0), at(ONE));
        assert!(n.fill[2] < d.fill[2]);
        assert!(n.fill[2] > n.fill[0]);
        assert!(n.sun[2] > n.sun[0] && n.sun[2] > cm(30));
        assert_eq!(n.stars, ONE);
        assert_eq!(d.stars, 0);
    }
}
