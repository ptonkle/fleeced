//! Smooth seeded noise for the hillside's patches, in whole numbers. A value
//! noise: a hash at each lattice point, blended with a smoothstep.

use crate::fx::{Fx, ONE};

/// A seeded hash of a lattice point, from 0 up to, not including, `ONE`.
pub fn lattice(seed: u32, x: i32, z: i32) -> Fx {
    let mut h = (x as u32)
        .wrapping_mul(0x9E37_79B1)
        .wrapping_add((z as u32).wrapping_mul(0x85EB_CA77))
        .wrapping_add(seed.wrapping_mul(0xC2B2_AE3D));
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 16) as Fx
}

/// The mottling's hash: x's and z's multipliers, the square's, how far the
/// result shifts down, and the largest value that leaves. All fit a uniform,
/// which reaches `grass.ground.frag` as a whole number of at most 32,767.
pub const MOTTLE_HASH: [u32; 3] = [31_337, 20_011, 28_411];
pub const MOTTLE_SHIFT: u32 = 17;
pub const MOTTLE_TOP: i32 = 32_767;

/// The sward's mottling at a lattice point. `grass.ground.frag` does these
/// same sums, so blades take the patch of ground they stand on.
pub fn mottle_hash(x: i32, z: i32) -> Fx {
    let q = (x as u32).wrapping_mul(MOTTLE_HASH[0]) ^ (z as u32).wrapping_mul(MOTTLE_HASH[1]);
    (q.wrapping_mul(q).wrapping_mul(MOTTLE_HASH[2]) >> MOTTLE_SHIFT) as Fx * 2
}

/// `mottle_hash` blended smoothly between lattice points.
pub fn blotch(x: Fx, z: Fx) -> Fx {
    let (cx, cz) = (x >> 16, z >> 16);
    let (ux, uz) = (smooth(x & 0xFFFF), smooth(z & 0xFFFF));
    let low = lerp(mottle_hash(cx, cz), mottle_hash(cx + 1, cz), ux);
    let high = lerp(mottle_hash(cx, cz + 1), mottle_hash(cx + 1, cz + 1), ux);
    lerp(low, high, uz)
}

/// 3u² - 2u³ for `u` from 0 to `ONE`.
pub fn smooth(u: Fx) -> Fx {
    let u = u.clamp(0, ONE) as i64;
    let u2 = (u * u) >> 16;
    ((u2 * (3 * ONE as i64 - 2 * u)) >> 16) as Fx
}

/// `t` of the way from `lo` to `hi`, smoothed: 0 at `lo`, `ONE` at `hi`.
pub fn band(lo: Fx, hi: Fx, v: Fx) -> Fx {
    if hi <= lo {
        return if v >= hi { ONE } else { 0 };
    }
    let span = (hi - lo) as i64;
    smooth((((v as i64 - lo as i64) << 16) / span).clamp(0, ONE as i64) as Fx)
}

/// `a` to `b`, `t` of the way.
pub fn lerp(a: Fx, b: Fx, t: Fx) -> Fx {
    a + (((b as i64 - a as i64) * t as i64) >> 16) as Fx
}

/// Noise from 0 to `ONE` with lattice points `1 << shift` apart in `Fx`
/// units, so shift 20 means 16 m.
pub fn value(seed: u32, x: Fx, z: Fx, shift: u32) -> Fx {
    let (cx, cz) = (x >> shift, z >> shift);
    let mask = (1i32 << shift) - 1;
    let frac = |v: Fx| (((v & mask) as i64) << 16 >> shift) as Fx;
    let (u, v) = (smooth(frac(x)), smooth(frac(z)));
    let near = lerp(lattice(seed, cx, cz), lattice(seed, cx + 1, cz), u);
    let far = lerp(lattice(seed, cx, cz + 1), lattice(seed, cx + 1, cz + 1), u);
    lerp(near, far, v)
}

/// `octaves` of noise, each half the size and half the strength of the last.
/// From 0 to `ONE`, and mostly near the middle.
pub fn fbm(seed: u32, x: Fx, z: Fx, shift: u32, octaves: u32) -> Fx {
    let mut total = 0i64;
    let mut whole = 0i64;
    let mut weight = ONE as i64;
    for i in 0..octaves {
        total += (value(seed.wrapping_add(i), x, z, shift - i) as i64 * weight) >> 16;
        whole += weight;
        weight /= 2;
    }
    ((total << 16) / whole) as Fx
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::{from_int, ratio};

    #[test]
    fn noise_stays_between_its_limits_and_depends_on_the_seed() {
        let mut changed = 0;
        for i in 0..2000 {
            let (x, z) = (from_int(i) / 7, from_int(i * 3 - 500) / 5);
            let a = value(1, x, z, 18);
            assert!((0..ONE).contains(&a), "{a}");
            assert!((0..=ONE).contains(&fbm(1, x, z, 20, 3)));
            changed += (a != value(2, x, z, 18)) as i32;
        }
        assert!(changed > 1900, "{changed}");
    }

    #[test]
    fn noise_is_smooth_across_a_lattice_line() {
        // one lattice cell is 4 m: 1 cm either side of a line is a tiny step
        let (line, step) = (from_int(8), ONE / 100);
        let z = ratio(3, 2);
        let (below, above) = (value(5, line - step, z, 18), value(5, line + step, z, 18));
        assert!((below - above).abs() < ONE / 200, "{below} {above}");
    }

    #[test]
    fn band_runs_from_zero_to_one_through_its_range() {
        assert_eq!(band(ONE, 2 * ONE, 0), 0);
        assert_eq!(band(ONE, 2 * ONE, 3 * ONE), ONE);
        assert_eq!(band(0, 2 * ONE, ONE), ONE / 2);
    }

    #[test]
    fn the_mottle_hash_is_the_shaders_sum() {
        // worked by hand in 32-bit integers, as uint arithmetic in the shader
        assert_eq!(mottle_hash(1, 0), 61102);
        assert_eq!(mottle_hash(0, 1), 58454);
        assert_eq!(mottle_hash(-3, 7), 45778);
    }

    #[test]
    fn a_blotch_runs_unbroken_across_a_lattice_line() {
        let z = ONE * 3 / 8;
        for cell in -3..3 {
            let line = cell * ONE;
            let step = blotch(line, z) - blotch(line - 1, z);
            assert!(step.abs() < 64, "cell {cell}: {step}");
        }
    }
}
