//! The checks that show a map is playable, kept beside the map so the tests
//! and the probe share one copy. Everything here asks the map's own
//! `push_out_as`, the same call bodies use, and never reads the numbers the
//! map was built from.
//!
//! The method is a flood over a 0.5 m grid. A grid point is open when a
//! circle of radius r there touches nothing. Two neighboring open points are
//! 0.5 m apart, and a circle moved between them stays at most 0.25 m from
//! one of them, so a chain of open points carries a circle of radius
//! r - 0.25: a route 2r - 0.5 wide at every point along it.

use crate::fx::{Fx, ONE};

use super::{solids, table, Map, Mover, P};

const STEP: Fx = ONE / 2;

/// What the grid guarantees below the radius it floods at.
pub const GRID_SLACK: Fx = ONE / 4;

/// The grid over the whole map, with each point open or shut.
pub struct Flood {
    cols: usize,
    rows: usize,
    open: Vec<bool>,
    reached: Vec<bool>,
}

fn point(col: usize, row: usize) -> P {
    (
        -solids::OVERHANG / 4 + (col as Fx) * STEP,
        -solids::OVERHANG / 4 + (row as Fx) * STEP,
    )
}

impl Flood {
    fn index(&self, p: P) -> usize {
        let col = ((p.0 + solids::OVERHANG / 4) / STEP) as usize;
        let row = ((p.1 + solids::OVERHANG / 4) / STEP) as usize;
        row * self.cols + col
    }

    /// Marks every point a circle of `radius` can stand on, for this mover,
    /// inside the map, and floods out from `from` through them.
    pub fn from(map: &Map, from: P, radius: Fx, mover: Mover) -> Flood {
        let margin = solids::OVERHANG / 2;
        let cols = ((map.length() + margin) / STEP) as usize + 1;
        let rows = ((map.width() + margin) / STEP) as usize + 1;
        let mut open = Vec::with_capacity(cols * rows);
        for row in 0..rows {
            for col in 0..cols {
                let p = point(col, row);
                open.push(map.inside(p.0, p.1) && map.push_out_as(p.0, p.1, radius, mover) == p);
            }
        }
        let mut flood = Flood {
            cols,
            rows,
            open,
            reached: vec![false; cols * rows],
        };
        flood.run(from);
        flood
    }

    /// Floods from the open point nearest `from`, within a meter or so.
    fn run(&mut self, from: P) {
        let start = self.index(from);
        let found = (0..=3i64)
            .flat_map(|r| (-r..=r).flat_map(move |dz| (-r..=r).map(move |dx| (dx, dz))))
            .map(|(dx, dz)| start as i64 + dz * self.cols as i64 + dx)
            .find(|&i| i >= 0 && (i as usize) < self.open.len() && self.open[i as usize]);
        let Some(first) = found else {
            return;
        };
        let mut stack = vec![first as usize];
        self.reached[first as usize] = true;
        while let Some(i) = stack.pop() {
            let (col, row) = (i % self.cols, i / self.cols);
            let mut visit = |j: usize, reached: &mut Vec<bool>| {
                if self.open[j] && !reached[j] {
                    reached[j] = true;
                    stack.push(j);
                }
            };
            if col > 0 {
                visit(i - 1, &mut self.reached);
            }
            if col + 1 < self.cols {
                visit(i + 1, &mut self.reached);
            }
            if row > 0 {
                visit(i - self.cols, &mut self.reached);
            }
            if row + 1 < self.rows {
                visit(i + self.cols, &mut self.reached);
            }
        }
    }

    /// Open points the flood reached.
    pub fn reached(&self) -> usize {
        self.reached.iter().filter(|&&r| r).count()
    }

    /// Is a reached point one step away, diagonals included?
    fn beside_reached(&self, i: usize) -> bool {
        let (col, row) = ((i % self.cols) as i64, (i / self.cols) as i64);
        (-1..=1).any(|dz| {
            (-1..=1).any(|dx| {
                let (c, r) = (col + dx, row + dz);
                let on_grid = c >= 0 && r >= 0 && c < self.cols as i64 && r < self.rows as i64;
                on_grid && self.reached[r as usize * self.cols + c as usize]
            })
        })
    }

    /// Open points cut off from the flood: pockets. In the tip of a wedge
    /// where two walls meet, the last point can join the open ground only
    /// by a diagonal, which a step between neighbors can't see, so a point
    /// a diagonal away from reached ground is no pocket.
    pub fn sealed(&self) -> usize {
        (0..self.open.len())
            .filter(|&i| self.open[i] && !self.reached[i] && !self.beside_reached(i))
            .count()
    }

    /// Reached points in each field, then in the Fold.
    pub fn reached_by_place(&self, map: &Map) -> [usize; 4] {
        let mut counts = [0; 4];
        for row in 0..self.rows {
            for col in 0..self.cols {
                if self.reached[row * self.cols + col] {
                    let p = point(col, row);
                    let place = map.field_at(p.0, p.1).unwrap_or(3);
                    counts[place] += 1;
                }
            }
        }
        counts
    }

    /// Did the flood reach the point nearest `p`?
    pub fn reaches(&self, p: P) -> bool {
        let i = self.index(p);
        let (col, row) = (i % self.cols, i / self.cols);
        // the corners of the grid square around p
        [(0, 0), (1, 0), (0, 1), (1, 1)].iter().any(|&(dx, dz)| {
            let (c, r) = (col + dx, row + dz);
            c < self.cols && r < self.rows && self.reached[r * self.cols + c]
        })
    }
}

/// The widest flood from the Fold's middle that still reaches all three
/// fields, to the nearest 5 cm, as the width of route it proves.
pub fn route_width(map: &Map) -> Fx {
    let reaches_all = |radius: Fx| {
        let flood = Flood::from(map, map.fold.center, radius, Mover::Sheep);
        flood.reached_by_place(map)[..3].iter().all(|&n| n > 0)
    };
    let (mut lo, mut hi) = (table::tenths(3), table::meters(6));
    while hi - lo > ONE / 40 {
        let mid = (lo + hi) / 2;
        if reaches_all(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    2 * (lo - GRID_SLACK)
}
