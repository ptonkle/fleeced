//! The hillside's height: a sampled grid and bicubic interpolation between
//! the samples, so a slope has no creases at the cell edges.

use serde::Serialize;

use crate::fx::{Fx, ONE};
use crate::rng::Rng;

use super::size::MapSize;
use super::table;

/// Heights on a square grid. `cell_shift` makes the spacing a power of two
/// of the Fx scale, so a lookup needs no division.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Heightfield {
    pub x0: Fx,
    pub z0: Fx,
    pub cell_shift: u32,
    pub cols: usize,
    pub rows: usize,
    pub samples: Vec<Fx>,
}

/// Catmull-Rom weights at fraction `u` (Q16), doubled. They sum to exactly
/// twice `ONE` whatever the rounding of `u2` and `u3`, so a level surface
/// stays level to the last bit.
fn weights(u: i64) -> [i64; 4] {
    let u2 = (u * u) >> 16;
    let u3 = (u2 * u) >> 16;
    [
        -u + 2 * u2 - u3,
        2 * ONE as i64 - 5 * u2 + 3 * u3,
        u + 4 * u2 - 3 * u3,
        u3 - u2,
    ]
}

impl Heightfield {
    fn sample(&self, col: i64, row: i64) -> i64 {
        let c = col.clamp(0, self.cols as i64 - 1) as usize;
        let r = row.clamp(0, self.rows as i64 - 1) as usize;
        self.samples[r * self.cols + c] as i64
    }

    /// The height at any point. Past the grid's edge it holds the edge.
    pub fn at(&self, x: Fx, z: Fx) -> Fx {
        let along = (x as i64 - self.x0 as i64).max(0);
        let across = (z as i64 - self.z0 as i64).max(0);
        let col = (along >> self.cell_shift).min(self.cols as i64);
        let row = (across >> self.cell_shift).min(self.rows as i64);
        let mask = (1i64 << self.cell_shift) - 1;
        let to_q16 = |v: i64| ((v & mask) << 16) >> self.cell_shift;
        let wx = weights(to_q16(along));
        let wz = weights(to_q16(across));
        let mut total = 0i64;
        for (j, wj) in wz.iter().enumerate() {
            let mut line = 0i64;
            for (i, wi) in wx.iter().enumerate() {
                line += wi * self.sample(col + i as i64 - 1, row + j as i64 - 1);
            }
            total += wj * (line >> 17);
        }
        (total >> 17) as Fx
    }

    /// Filled by `height(x, z)` at every grid point.
    fn filled(
        corner: (Fx, Fx),
        shift: u32,
        cols: usize,
        rows: usize,
        mut height: impl FnMut(Fx, Fx) -> Fx,
    ) -> Self {
        let mut samples = Vec::with_capacity(cols * rows);
        for row in 0..rows {
            for col in 0..cols {
                let x = corner.0 + ((col as i32) << shift);
                let z = corner.1 + ((row as i32) << shift);
                samples.push(height(x, z));
            }
        }
        Heightfield {
            x0: corner.0,
            z0: corner.1,
            cell_shift: shift,
            cols,
            rows,
            samples,
        }
    }
}

/// One octave of rolling: random heights on a coarse lattice.
fn octave(rng: &mut Rng, size: MapSize, shift: u32, tallest: Fx) -> Heightfield {
    let m = table::TERRAIN_MARGIN;
    let cols = ((size.length() + 2 * m) >> shift) as usize + 2;
    let rows = ((size.width() + 2 * m) >> shift) as usize + 2;
    Heightfield::filled((-m, -m), shift, cols, rows, |_, _| rng.between(-tallest, tallest))
}

/// How much of the rolling stands at x: all of it, fading to a quarter over
/// the fade, so the Fold sits level. Smoothstep, in Q16.
fn roll_weight(x: Fx) -> i64 {
    let span = (table::ROLL_FADE_TO - table::ROLL_FADE_FROM) as i64;
    let t = ((x as i64 - table::ROLL_FADE_FROM as i64).clamp(0, span) << 16) / span;
    let smooth = (((t * t) >> 16) * (3 * ONE as i64 - 2 * t)) >> 16;
    ONE as i64 - smooth * 3 / 4
}

/// The steady climb of the hill at x, before any rolling. The same slope on
/// every Map size, because jumps are tuned against it.
pub fn ramp(x: Fx) -> Fx {
    (table::RISE as i64 * x as i64 / table::LENGTH as i64) as Fx
}

/// The hillside for this seed.
pub fn build(rng: &mut Rng, size: MapSize) -> Heightfield {
    let octaves: Vec<Heightfield> = table::ROLL_OCTAVES
        .iter()
        .map(|&(shift, tallest)| octave(rng, size, shift, tallest))
        .collect();
    let m = table::TERRAIN_MARGIN;
    let shift = table::TERRAIN_CELL_SHIFT;
    let cols = ((size.length() + 2 * m) >> shift) as usize + 1;
    let rows = ((size.width() + 2 * m) >> shift) as usize + 1;
    Heightfield::filled((-m, -m), shift, cols, rows, |x, z| {
        let roll: i64 = octaves.iter().map(|o| o.at(x, z) as i64).sum();
        ramp(x) + ((roll * roll_weight(size.from_top(x))) >> 16) as Fx
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::{from_int, ratio};

    fn plane() -> Heightfield {
        // height = x/2 + 3z on a 4 m grid
        Heightfield::filled((0, 0), 18, 8, 8, |x, z| x / 2 + 3 * z)
    }

    #[test]
    fn terrain_passes_through_its_samples() {
        let f = Heightfield::filled((0, 0), 18, 8, 8, |x, z| (x ^ z) & 0xfffff);
        // a grid point's weights are 0, 2, 0, 0: the sample itself
        for (col, row) in [(1, 1), (3, 2), (6, 6)] {
            let (x, z) = (from_int(4 * col), from_int(4 * row));
            assert_eq!(f.at(x, z), f.samples[row as usize * 8 + col as usize]);
        }
    }

    #[test]
    fn terrain_reproduces_a_plane_between_samples() {
        let f = plane();
        // at (9.5, 10.25): 9.5/2 + 3 * 10.25 = 35.5
        let got = f.at(ratio(19, 2), ratio(41, 4));
        assert!((got - ratio(71, 2)).abs() <= 4, "{got}");
    }

    #[test]
    fn terrain_past_the_grid_holds_the_edge() {
        let f = plane();
        // the far corner sample is (28, 28): 14 + 84 = 98
        let far = from_int(30000);
        assert_eq!(f.at(far, far), from_int(98));
        assert_eq!(f.at(-far, -far), 0);
    }
}
