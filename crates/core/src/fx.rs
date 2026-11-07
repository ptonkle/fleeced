//! Q16.16 fixed point: the reference default's answer to "no floats in the
//! core" (BUILDER-SPEC.md, D2). Part of the realtime kit
//! (`kits/realtime/README.md`).
//!
//! A value is an `i32` holding the number times 65536. Add and subtract
//! with `+` and `-`; multiply and divide through `mul` and `div`, which
//! widen to `i64` and saturate, so a product never wraps silently. The same
//! inputs give the same bits on every machine, which is what replays,
//! ghosts and lockstep play need.
//!
//! Angles are in turns: `TURN` is a full circle. `sin` and `cos` use
//! Bhaskara's rational approximation, within about 0.002 of the true value,
//! which is enough for movement and aiming. A project that needs more adds
//! a table built from integers and a test against hand-computed values.

pub type Fx = i32;

pub const SHIFT: u32 = 16;
pub const ONE: Fx = 1 << SHIFT;
pub const HALF: Fx = ONE / 2;

/// A full turn, a half and a quarter, in angle units.
pub const TURN: i32 = 1 << 16;
pub const HALF_TURN: i32 = TURN / 2;
pub const QUARTER_TURN: i32 = TURN / 4;

fn saturate(v: i64) -> Fx {
    v.clamp(i32::MIN as i64, i32::MAX as i64) as Fx
}

/// A whole number as a fixed-point value. Holds -32768 to 32767.
pub const fn from_int(n: i32) -> Fx {
    n << SHIFT
}

/// Rounds toward negative infinity.
pub const fn floor(x: Fx) -> i32 {
    x >> SHIFT
}

/// Rounds half up.
pub const fn round(x: Fx) -> i32 {
    (x + HALF) >> SHIFT
}

/// `num / den` as a fixed-point value. `den` must not be zero.
pub fn ratio(num: i32, den: i32) -> Fx {
    saturate(((num as i64) << SHIFT) / den as i64)
}

pub fn mul(a: Fx, b: Fx) -> Fx {
    saturate((a as i64 * b as i64) >> SHIFT)
}

/// `a / b`. `b` must not be zero; a rule that can divide by zero decides
/// what that means before it calls this.
pub fn div(a: Fx, b: Fx) -> Fx {
    saturate(((a as i64) << SHIFT) / b as i64)
}

fn isqrt(n: u64) -> u64 {
    if n < 2 {
        return n;
    }
    // Newton's method from a guess above the root
    let mut x = n;
    let mut y = x.div_ceil(2);
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// The square root of a non-negative value; negative values give 0.
pub fn sqrt(x: Fx) -> Fx {
    if x <= 0 {
        return 0;
    }
    saturate(isqrt((x as u64) << SHIFT) as i64)
}

/// The length of the vector (x, y).
pub fn length(x: Fx, y: Fx) -> Fx {
    let sq = x as i64 * x as i64 + y as i64 * y as i64;
    saturate(isqrt(sq as u64) as i64)
}

/// The sine of an angle in turns.
pub fn sin(angle: i32) -> Fx {
    let a = angle.rem_euclid(TURN) as i64;
    let (t, sign) = if a < HALF_TURN as i64 {
        (a, 1)
    } else {
        (a - HALF_TURN as i64, -1)
    };
    let h = HALF_TURN as i64;
    let p = t * (h - t);
    let value = ((16 * p) << SHIFT) / (5 * h * h - 4 * p);
    sign * value as Fx
}

pub fn cos(angle: i32) -> Fx {
    sin(angle + QUARTER_TURN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn products_and_quotients_keep_sixteen_fraction_bits() {
        assert_eq!(mul(from_int(3), HALF), ratio(3, 2));
        assert_eq!(mul(ratio(-5, 2), from_int(2)), from_int(-5));
        assert_eq!(div(from_int(7), from_int(2)), ratio(7, 2));
        assert_eq!(div(ONE, from_int(3)), 21845, "1/3 truncates to 21845/65536");
    }

    #[test]
    fn an_overflowing_product_saturates_rather_than_wrapping() {
        assert_eq!(mul(from_int(30000), from_int(30000)), i32::MAX);
        assert_eq!(mul(from_int(-30000), from_int(30000)), i32::MIN);
    }

    #[test]
    fn floor_and_round_go_the_stated_way() {
        assert_eq!(floor(ratio(-1, 2)), -1);
        assert_eq!(round(ratio(-1, 2)), 0);
        assert_eq!(round(ratio(5, 2)), 3);
        assert_eq!(floor(ratio(5, 2)), 2);
    }

    #[test]
    fn square_roots_and_lengths_match_hand_computed_values() {
        assert_eq!(sqrt(from_int(9)), from_int(3));
        assert_eq!(sqrt(ratio(1, 4)), HALF);
        assert_eq!(sqrt(from_int(-4)), 0);
        assert_eq!(length(from_int(3), from_int(4)), from_int(5));
        // sqrt(2) = 1.41421..., times 65536 = 92681.9
        assert_eq!(sqrt(from_int(2)), 92681);
    }

    #[test]
    fn sine_and_cosine_land_within_the_stated_error() {
        let close = |got: Fx, want: Fx| (got - want).abs() <= ONE / 500;
        assert_eq!(sin(0), 0);
        assert_eq!(sin(QUARTER_TURN), ONE);
        assert_eq!(sin(HALF_TURN), 0);
        assert_eq!(sin(-QUARTER_TURN), -ONE);
        assert_eq!(cos(0), ONE);
        // sin(45 degrees) = 0.70711, times 65536 = 46341
        assert!(close(sin(TURN / 8), 46341), "{}", sin(TURN / 8));
        // sin(30 degrees) = 0.5
        assert!(close(sin(TURN / 12), HALF), "{}", sin(TURN / 12));
        assert_eq!(
            sin(TURN + QUARTER_TURN),
            ONE,
            "a whole turn more is the same angle"
        );
    }
}
