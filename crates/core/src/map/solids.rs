//! Collision. Every solid is a capsule: a straight line with a thickness,
//! and a circle is a line of no length. A coarse grid of cells lists the
//! solids near each cell, so a body in the open touches nothing and costs
//! next to nothing, which matters with 400 sheep and rollback replays.

use crate::fx::Fx;

use super::table;
use super::P;

/// What a solid is, which decides who it stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Outer,
    Inner,
    FoldWall,
    /// A line across the Fold's gap, solid only to those shut out or in.
    FoldGap,
    Trunk,
    /// The base of the numbered cairn.
    Cairn(u8),
    /// A low stone: `top` says how high, and a jump can land on it.
    Slab,
}

/// A line from a, of direction e, thickened by `half` on every side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Solid {
    ax: Fx,
    az: Fx,
    ex: Fx,
    ez: Fx,
    /// |e|², in the coarser units `nearest` works in.
    len2: i64,
    pub half: Fx,
    /// A wall's or slab's height above the ground; 0 on anything else.
    pub top: Fx,
    pub part: Part,
}

/// Projection works in 1/4096 m, so a dot product of map-sized lines can
/// take a further 16 bits without leaving an i64.
const COARSE: u32 = 4;

impl Solid {
    pub fn line(a: P, b: P, half: Fx, top: Fx, part: Part) -> Solid {
        let (ex, ez) = (b.0 - a.0, b.1 - a.1);
        let (cx, cz) = ((ex >> COARSE) as i64, (ez >> COARSE) as i64);
        Solid {
            ax: a.0,
            az: a.1,
            ex,
            ez,
            len2: cx * cx + cz * cz,
            half,
            top,
            part,
        }
    }

    pub fn circle(center: P, radius: Fx, part: Part) -> Solid {
        Solid::line(center, center, radius, 0, part)
    }

    /// The point of the line nearest (x, z).
    pub fn nearest(&self, x: Fx, z: Fx) -> P {
        if self.len2 == 0 {
            return (self.ax, self.az);
        }
        let px = ((x - self.ax) >> COARSE) as i64;
        let pz = ((z - self.az) >> COARSE) as i64;
        let dot = px * (self.ex >> COARSE) as i64 + pz * (self.ez >> COARSE) as i64;
        if dot <= 0 {
            return (self.ax, self.az);
        }
        if dot >= self.len2 {
            return (self.ax + self.ex, self.az + self.ez);
        }
        let t = (dot << 16) / self.len2;
        (
            self.ax + ((self.ex as i64 * t) >> 16) as Fx,
            self.az + ((self.ez as i64 * t) >> 16) as Fx,
        )
    }

    /// The unit normal to the line's left, facing into the map on a
    /// counterclockwise outer wall.
    pub fn left(&self) -> P {
        if self.ex == 0 && self.ez == 0 {
            return (crate::fx::ONE, 0);
        }
        let n = crate::fx::length(self.ex, self.ez);
        (crate::fx::div(-self.ez, n), crate::fx::div(self.ex, n))
    }
}

/// The floor of the square root, from a guess at or above the root: a few
/// rounds of Newton, where `fx::length` takes dozens from its guess of n.
pub fn root(n: u64) -> u64 {
    if n < 2 {
        return n;
    }
    let bits = 64 - n.leading_zeros();
    let mut x = 1u64 << bits.div_ceil(2);
    loop {
        let y = (x + n / x) / 2;
        if y >= x {
            return x;
        }
        x = y;
    }
}

/// Does the polygon hold the point? Crossing count, in exact integers.
pub fn polygon_contains(polygon: &[P], x: Fx, z: Fx) -> bool {
    let mut inside = false;
    let mut prev = polygon[polygon.len() - 1];
    for &cur in polygon {
        if (cur.1 > z) != (prev.1 > z) {
            let dz = prev.1 as i64 - cur.1 as i64;
            let left = (x as i64 - cur.0 as i64) * dz;
            let right = (prev.0 as i64 - cur.0 as i64) * (z as i64 - cur.1 as i64);
            if (left < right) == (dz > 0) {
                inside = !inside;
            }
        }
        prev = cur;
    }
    inside
}

/// Which side of the outer wall a whole cell lies on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    Inside,
    Outside,
    /// The outer wall is near: ask point by point.
    Mixed,
}

/// Cells are 8 m, as a power of two of the Fx scale.
const CELL_SHIFT: u32 = 19;
/// The grid overhangs the map by this much on every side.
pub const OVERHANG: Fx = table::meters(16);
/// A cell lists every solid within this distance of the cell, so a query
/// out to this radius needs no other solid.
pub const REACH: Fx = table::meters(6);

/// The grid of cells, in flat lists. `Default` gives an empty one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Solids {
    pub list: Vec<Solid>,
    cols: i32,
    rows: i32,
    starts: Vec<u32>,
    items: Vec<u16>,
    zones: Vec<Zone>,
}

impl Solids {
    /// Builds the cells for these solids. `outer` is the outer wall.
    pub fn new(list: Vec<Solid>, outer: &[P], length: Fx, width: Fx) -> Solids {
        let cols = ((length + 2 * OVERHANG) >> CELL_SHIFT) + 1;
        let rows = ((width + 2 * OVERHANG) >> CELL_SHIFT) + 1;
        let half_cell = 1i32 << (CELL_SHIFT - 1);
        // half the cell's diagonal: 4 m times the square root of 2, rounded up
        let corner = table::tenths(57);
        let count = (cols * rows) as usize;
        let mut starts = Vec::with_capacity(count + 1);
        let mut items = Vec::new();
        let mut zones = Vec::with_capacity(count);
        starts.push(0);
        for row in 0..rows {
            for col in 0..cols {
                let x = -OVERHANG + (col << CELL_SHIFT) + half_cell;
                let z = -OVERHANG + (row << CELL_SHIFT) + half_cell;
                let mut outer_near = false;
                for (id, s) in list.iter().enumerate() {
                    let (qx, qz) = s.nearest(x, z);
                    let (dx, dz) = ((x - qx) as i64, (z - qz) as i64);
                    let far = (corner + REACH + s.half) as i64;
                    if dx * dx + dz * dz <= far * far {
                        items.push(id as u16);
                        outer_near |= s.part == Part::Outer;
                    }
                }
                starts.push(items.len() as u32);
                zones.push(match (outer_near, polygon_contains(outer, x, z)) {
                    (true, _) => Zone::Mixed,
                    (false, true) => Zone::Inside,
                    (false, false) => Zone::Outside,
                });
            }
        }
        Solids {
            list,
            cols,
            rows,
            starts,
            items,
            zones,
        }
    }

    fn cell(&self, x: Fx, z: Fx) -> usize {
        let col = ((x + OVERHANG) >> CELL_SHIFT).clamp(0, self.cols - 1);
        let row = ((z + OVERHANG) >> CELL_SHIFT).clamp(0, self.rows - 1);
        (row * self.cols + col) as usize
    }

    /// The solids near (x, z).
    pub fn near(&self, x: Fx, z: Fx) -> &[u16] {
        let c = self.cell(x, z);
        &self.items[self.starts[c] as usize..self.starts[c + 1] as usize]
    }

    pub fn zone(&self, x: Fx, z: Fx) -> Zone {
        self.zones[self.cell(x, z)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::from_int;

    #[test]
    fn the_nearest_point_on_a_line_clamps_to_its_ends() {
        // a line along x from (10, 5) to (30, 5)
        let s = Solid::line(
            (from_int(10), from_int(5)),
            (from_int(30), from_int(5)),
            0,
            0,
            Part::Inner,
        );
        // straight above the line's midpoint
        assert_eq!(s.nearest(from_int(20), from_int(9)), (from_int(20), from_int(5)));
        // before the start, and past the end
        assert_eq!(s.nearest(from_int(2), from_int(9)), (from_int(10), from_int(5)));
        assert_eq!(s.nearest(from_int(40), from_int(1)), (from_int(30), from_int(5)));
    }

    #[test]
    fn a_polygon_holds_points_inside_and_not_outside() {
        // a square with a bite out of its right side: an L-free C shape
        let m = |x: i32, z: i32| (from_int(x), from_int(z));
        let c = [
            m(0, 0),
            m(10, 0),
            m(10, 4),
            m(4, 4),
            m(4, 6),
            m(10, 6),
            m(10, 10),
            m(0, 10),
        ];
        assert!(polygon_contains(&c, from_int(2), from_int(5)));
        assert!(polygon_contains(&c, from_int(8), from_int(2)));
        assert!(!polygon_contains(&c, from_int(8), from_int(5)), "in the bite");
        assert!(!polygon_contains(&c, from_int(-1), from_int(5)));
        assert!(!polygon_contains(&c, from_int(12), from_int(2)));
    }

    #[test]
    fn root_is_the_floor_of_the_square_root() {
        assert_eq!(root(0), 0);
        assert_eq!(root(1), 1);
        assert_eq!(root(15), 3);
        assert_eq!(root(16), 4);
        assert_eq!(root(17), 4);
        // 65536 squared is 2^32
        assert_eq!(root(1 << 32), 65536);
        assert_eq!(root((1 << 32) - 1), 65535);
    }
}
