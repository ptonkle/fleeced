//! Vectors and matrices in Q16.16 for drawing. y is up. A heading (yaw) of 0
//! faces +z, and a positive yaw turns toward +x. (Ported from Ape Race.)

use crate::fx::{cos, div, mul, sin, ONE, QUARTER_TURN, SHIFT, TURN};

/// Degrees to angle units.
pub const fn deg(d: i32) -> i32 {
    ((d as i64 * TURN as i64) / 360) as i32
}

/// Hundredths: `cm(150)` is 1.5.
pub const fn cm(n: i32) -> i32 {
    ((n as i64 * ONE as i64) / 100) as i32
}

/// Thousandths: `mm(1500)` is 1.5.
pub const fn mm(n: i32) -> i32 {
    ((n as i64 * ONE as i64) / 1000) as i32
}

/// Linear blend: `t = 0` gives `a`, `t = ONE` gives `b`.
pub fn lerp(a: i32, b: i32, t: i32) -> i32 {
    a + (((b as i64 - a as i64) * t as i64) >> SHIFT) as i32
}

/// 0 at or below `lo`, ONE at or above `hi`, an S-curve between.
pub fn smooth(lo: i32, hi: i32, x: i32) -> i32 {
    let t = div((x - lo).clamp(0, hi - lo), hi - lo);
    mul(mul(t, t), 3 * ONE - 2 * t)
}

/// The angle of the point (x, y) from the +x axis, in turns, within about
/// 0.1 degrees: a polynomial arctangent on the first octant, then folded.
pub fn atan2(y: i32, x: i32) -> i32 {
    if x == 0 && y == 0 {
        return 0;
    }
    let (ax, ay) = ((x as i64).abs(), (y as i64).abs());
    let (num, den, swap) = if ay <= ax { (ay, ax, false) } else { (ax, ay, true) };
    let t = (num << SHIFT) / den;
    // atan(t) = pi/4 t - t(t - 1)(0.2447 + 0.0663 t), radians
    let one = ONE as i64;
    let base = t * (TURN as i64 / 8) / one;
    let bend = (t * (t - one) / one) * (2552 + 691 * t / one) / one;
    let mut a = base - bend;
    if swap {
        a = QUARTER_TURN as i64 - a;
    }
    if x < 0 {
        a = (TURN / 2) as i64 - a;
    }
    if y < 0 {
        a = -a;
    }
    a as i32
}

fn isqrt(n: u128) -> u128 {
    // bit by bit: no division, which is slow on 128 bits in WebAssembly
    let mut rest = n;
    let mut root = 0u128;
    let mut bit = 1u128 << ((127 - n.leading_zeros().min(127)) & !1);
    while bit != 0 {
        if rest >= root + bit {
            rest -= root + bit;
            root = (root >> 1) + bit;
        } else {
            root >>= 1;
        }
        bit >>= 2;
    }
    root
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct V3 {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

pub const fn v3(x: i32, y: i32, z: i32) -> V3 {
    V3 { x, y, z }
}

impl V3 {
    pub const ZERO: V3 = v3(0, 0, 0);
    pub const UP: V3 = v3(0, ONE, 0);

    pub(crate) fn add(self, o: V3) -> V3 {
        v3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
    pub(crate) fn sub(self, o: V3) -> V3 {
        v3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
    pub fn scale(self, s: i32) -> V3 {
        v3(mul(self.x, s), mul(self.y, s), mul(self.z, s))
    }
    /// Each axis scaled by its own factor.
    pub fn times(self, s: V3) -> V3 {
        v3(mul(self.x, s.x), mul(self.y, s.y), mul(self.z, s.z))
    }
    pub(crate) fn neg(self) -> V3 {
        v3(-self.x, -self.y, -self.z)
    }
    pub fn dot(self, o: V3) -> i32 {
        let s = self.x as i64 * o.x as i64 + self.y as i64 * o.y as i64 + self.z as i64 * o.z as i64;
        (s >> SHIFT).clamp(i32::MIN as i64, i32::MAX as i64) as i32
    }
    pub fn cross(self, o: V3) -> V3 {
        let c =
            |a: i32, b: i32, c: i32, d: i32| ((a as i64 * b as i64 - c as i64 * d as i64) >> SHIFT) as i32;
        v3(
            c(self.y, o.z, self.z, o.y),
            c(self.z, o.x, self.x, o.z),
            c(self.x, o.y, self.y, o.x),
        )
    }
    fn len2(self) -> u64 {
        let sq = |a: i32| (a as i64 * a as i64) as u64;
        sq(self.x) + sq(self.y) + sq(self.z)
    }
    pub(crate) fn len(self) -> i32 {
        isqrt(self.len2() as u128) as i32
    }
    /// A vector of length `ONE` the same way, or zero for zero.
    pub fn norm(self) -> V3 {
        let s = self.len2();
        if s == 0 {
            return V3::ZERO;
        }
        // the length with 32 more bits, so short vectors keep their direction
        let l = isqrt((s as u128) << 32) as i64;
        let c = |a: i32| (((a as i64) << 32) / l) as i32;
        v3(c(self.x), c(self.y), c(self.z))
    }
    pub fn lerp(self, o: V3, t: i32) -> V3 {
        v3(lerp(self.x, o.x, t), lerp(self.y, o.y, t), lerp(self.z, o.z, t))
    }
    pub fn dist(self, o: V3) -> i32 {
        self.sub(o).len()
    }
}

/// The unit vector for a heading and a pitch.
pub fn facing(yaw: i32, pitch: i32) -> V3 {
    let cp = cos(pitch);
    v3(mul(sin(yaw), cp), sin(pitch), mul(cos(yaw), cp))
}

/// An affine transform: three columns (the images of the unit axes) and a
/// translation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Affine {
    pub c0: V3,
    pub c1: V3,
    pub c2: V3,
    pub t: V3,
}

impl Affine {
    pub const IDENTITY: Affine = Affine {
        c0: v3(ONE, 0, 0),
        c1: v3(0, ONE, 0),
        c2: v3(0, 0, ONE),
        t: V3::ZERO,
    };

    pub fn translate(t: V3) -> Affine {
        Affine {
            t,
            ..Affine::IDENTITY
        }
    }
    pub fn scale(s: V3) -> Affine {
        Affine {
            c0: v3(s.x, 0, 0),
            c1: v3(0, s.y, 0),
            c2: v3(0, 0, s.z),
            t: V3::ZERO,
        }
    }
    pub fn rot_y(a: i32) -> Affine {
        let (s, c) = (sin(a), cos(a));
        Affine {
            c0: v3(c, 0, -s),
            c1: v3(0, ONE, 0),
            c2: v3(s, 0, c),
            t: V3::ZERO,
        }
    }
    pub fn rot_x(a: i32) -> Affine {
        let (s, c) = (sin(a), cos(a));
        Affine {
            c0: v3(ONE, 0, 0),
            c1: v3(0, c, s),
            c2: v3(0, -s, c),
            t: V3::ZERO,
        }
    }
    pub fn rot_z(a: i32) -> Affine {
        let (s, c) = (sin(a), cos(a));
        Affine {
            c0: v3(c, s, 0),
            c1: v3(-s, c, 0),
            c2: v3(0, 0, ONE),
            t: V3::ZERO,
        }
    }
    pub fn apply(&self, p: V3) -> V3 {
        self.apply_dir(p).add(self.t)
    }
    pub fn apply_dir(&self, d: V3) -> V3 {
        self.c0.scale(d.x).add(self.c1.scale(d.y)).add(self.c2.scale(d.z))
    }
    /// This transform after `o`: `a.then(&b).apply(p) == a.apply(b.apply(p))`.
    pub fn then(&self, o: &Affine) -> Affine {
        Affine {
            c0: self.apply_dir(o.c0),
            c1: self.apply_dir(o.c1),
            c2: self.apply_dir(o.c2),
            t: self.apply(o.t),
        }
    }
    /// A rotation `r` about the point `pivot`.
    pub fn about(pivot: V3, r: &Affine) -> Affine {
        Affine::translate(pivot)
            .then(r)
            .then(&Affine::translate(pivot.neg()))
    }
}

/// A 4×4 matrix, column-major (as WebGL takes it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mat4(pub [i32; 16]);

impl Mat4 {
    pub fn get(&self, row: usize, col: usize) -> i32 {
        self.0[col * 4 + row]
    }

    pub fn mul(&self, o: &Mat4) -> Mat4 {
        let mut m = [0i32; 16];
        for col in 0..4 {
            for row in 0..4 {
                let mut s: i64 = 0;
                for k in 0..4 {
                    s += self.get(row, k) as i64 * o.get(k, col) as i64;
                }
                m[col * 4 + row] = (s >> SHIFT) as i32;
            }
        }
        Mat4(m)
    }

    /// A perspective projection: vertical field of view `fov` (turns),
    /// `aspect` = width / height.
    pub fn perspective(fov: i32, aspect: i32, near: i32, far: i32) -> Mat4 {
        let half = fov / 2;
        let f = div(cos(half), sin(half));
        let mut m = [0i32; 16];
        m[0] = div(f, aspect);
        m[5] = f;
        m[10] = div(far + near, near - far);
        m[11] = -ONE;
        m[14] = ((2 * far as i64 * near as i64) / (near - far) as i64) as i32;
        Mat4(m)
    }

    /// The view transform of a camera at `eye` looking along `fwd`.
    pub fn look(eye: V3, fwd: V3, up: V3) -> Mat4 {
        let f = fwd.norm();
        let s = f.cross(up).norm();
        let u = s.cross(f);
        let mut m = [0i32; 16];
        m[0] = s.x;
        m[4] = s.y;
        m[8] = s.z;
        m[1] = u.x;
        m[5] = u.y;
        m[9] = u.z;
        m[2] = -f.x;
        m[6] = -f.y;
        m[10] = -f.z;
        m[12] = -s.dot(eye);
        m[13] = -u.dot(eye);
        m[14] = f.dot(eye);
        m[15] = ONE;
        Mat4(m)
    }

    /// `M · (p, 1)` as `[x, y, z, w]`.
    pub fn project(&self, p: V3) -> [i32; 4] {
        let mut out = [0i32; 4];
        for (row, o) in out.iter_mut().enumerate() {
            let s = self.get(row, 0) as i64 * p.x as i64
                + self.get(row, 1) as i64 * p.y as i64
                + self.get(row, 2) as i64 * p.z as i64;
            *o = ((s >> SHIFT) + self.get(row, 3) as i64) as i32;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::from_int as int;

    #[test]
    fn a_unit_vector_has_length_one_at_any_scale() {
        for v in [
            v3(3, 4, 0),
            v3(int(300), int(-40), int(7)),
            v3(mm(1), mm(2), mm(-2)),
        ] {
            assert!((v.norm().len() - ONE).abs() < 8, "{v:?}");
        }
        assert_eq!(V3::ZERO.norm(), V3::ZERO);
    }

    #[test]
    fn atan2_lands_on_the_quarter_turns() {
        assert_eq!(atan2(0, ONE), 0);
        assert_eq!(atan2(ONE, 0), QUARTER_TURN);
        assert_eq!(atan2(-ONE, 0), -QUARTER_TURN);
        // 45 degrees within a tenth of a degree
        assert!((atan2(ONE, ONE) - deg(45)).abs() < deg(1) / 10 + 1);
    }

    #[test]
    fn a_rotation_about_a_pivot_leaves_the_pivot_still() {
        let p = v3(int(1), int(2), int(3));
        let m = Affine::about(p, &Affine::rot_x(deg(40)));
        let at = m.apply(p);
        assert!(at.dist(p) < 4, "{at:?}");
    }

    #[test]
    fn then_applies_the_second_transform_first() {
        let m = Affine::translate(v3(ONE, 0, 0)).then(&Affine::scale(v3(2 * ONE, 2 * ONE, 2 * ONE)));
        assert_eq!(m.apply(v3(ONE, 0, 0)), v3(3 * ONE, 0, 0));
    }

    #[test]
    fn the_camera_looks_down_minus_z_in_view_space() {
        let m = Mat4::look(v3(0, 0, int(5)), v3(0, 0, -ONE), V3::UP);
        let [_, _, z, w] = m.project(V3::ZERO);
        assert_eq!(z, int(-5), "five meters ahead");
        assert_eq!(w, ONE);
        // two meters nearer is three ahead: the axis itself points the right way
        assert_eq!(m.project(v3(0, 0, int(2)))[2], int(-3));
    }
}
