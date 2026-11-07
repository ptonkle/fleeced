//! The ground: a grid of heights, normals and colors, one per match.
//!
//! Inside the walls the turf is bright, grazed and patchy. Outside, the rough
//! hill the dyke keeps out goes tawny, with bracken and heather. The ground
//! wears to earth where sheep funnel through a gap and under the Fold's wall,
//! and goes darker along each wall's foot where they can't graze.

use crate::fx::{self, Fx, ONE};
use crate::map::{table, Map, P};

use super::cm;
use super::noise::{band, fbm, value};
use super::palette::{self as pal, mix, pct, Tone};

/// The grid's spacing is 2 m, as a power of two of the `Fx` scale.
pub const CELL_SHIFT: u32 = 17;
pub const CELL: Fx = 1 << CELL_SHIFT;
/// Percent of a measured color the ground is drawn in.
pub const LIT: i32 = 80;
/// Percent of the moor that heather or bracken can take at their thickest.
const MOOR_MOST: i32 = 55;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub x0: Fx,
    pub z0: Fx,
    /// The map's own length and width, without the margin.
    pub length: Fx,
    pub width: Fx,
    pub cols: usize,
    pub rows: usize,
    pub heights: Vec<Fx>,
    pub normals: Vec<[Fx; 3]>,
    pub colors: Vec<Tone>,
    /// How worn each corner is, `ONE` for bare earth.
    pub worn: Vec<Fx>,
}

/// The distance from `p` to the line from `a` to `b`, in `Fx`.
pub fn to_segment(p: P, a: P, b: P) -> Fx {
    const COARSE: u32 = 4;
    let (ex, ez) = (((b.0 - a.0) >> COARSE) as i64, ((b.1 - a.1) >> COARSE) as i64);
    let (px, pz) = (((p.0 - a.0) >> COARSE) as i64, ((p.1 - a.1) >> COARSE) as i64);
    let len2 = ex * ex + ez * ez;
    if len2 == 0 {
        return fx::length(p.0 - a.0, p.1 - a.1);
    }
    let dot = (px * ex + pz * ez).clamp(0, len2);
    let qx = (ex * dot / len2) << COARSE;
    let qz = (ez * dot / len2) << COARSE;
    fx::length(p.0 - a.0 - qx as Fx, p.1 - a.1 - qz as Fx)
}

/// The grazed turf: soft patches of deep and lush green, thin pale places.
fn turf(seed: u32, x: Fx, z: Fx) -> Tone {
    let broad = fbm(seed, x, z, 21, 3);
    let green = if broad < ONE / 2 {
        mix(pal::TURF_DEEP, pal::TURF, band(cm(32), cm(50), broad))
    } else {
        mix(pal::TURF, pal::TURF_LUSH, band(cm(50), cm(68), broad))
    };
    let thin = band(cm(56), cm(74), fbm(seed + 11, x, z, 20, 3));
    mix(green, pal::TURF_BLEACHED, thin * 3 / 5)
}

/// The rough hill past the outer wall: bent grass with heather and bracken
/// worked into it. Neither ever takes over: each fades in over tens of meters
/// and a smaller noise frays its edge, as in `pine-broomy`.
pub fn moor(seed: u32, x: Fx, z: Fx) -> Tone {
    let pale = band(cm(30), cm(70), fbm(seed + 23, x, z, 21, 3));
    let grass = mix(pal::MOOR, pal::MOOR_PALE, pale * 3 / 5);
    let fray = fbm(seed + 61, x, z, 20, 2) - ONE / 2;
    let bracken = band(cm(40), cm(76), fbm(seed + 37, x, z, 21, 2) + fray / 3);
    let heather = band(cm(40), cm(74), fbm(seed + 53, x, z, 21, 3) + fray / 3);
    mix(
        mix(grass, pal::BRACKEN, bracken * MOOR_MOST / 100),
        pal::HEATHER,
        heather * MOOR_MOST / 100,
    )
}

/// How worn the ground is at (x, z), `ONE` for bare earth.
fn wear(map: &Map, seed: u32, x: Fx, z: Fx) -> Fx {
    // the edge of a worn patch wanders by up to a meter and a half
    let wander = fx::mul(value(seed + 71, x, z, 18) - ONE / 2, cm(300));
    let mut most = 0;
    for g in map.gaps.iter().chain(std::iter::once(&map.fold.gap)) {
        let d = to_segment((x, z), g.a, g.b) + wander;
        most = most.max(band(cm(70), cm(460), cm(520) - d));
    }
    if map.in_fold(x, z) {
        let patchy = band(cm(35), cm(70), fbm(seed + 83, x, z, 20, 2));
        most = most.max(cm(30) + fx::mul(patchy, cm(34)));
    }
    most
}

/// The ground's color at one point, and how worn it is.
fn color_at(map: &Map, seed: u32, x: Fx, z: Fx) -> (Tone, Fx) {
    if !map.inside(x, z) {
        return (moor(seed, x, z), 0);
    }
    let green = turf(seed, x, z);
    let nearest_wall = map
        .walls
        .iter()
        .map(|w| to_segment((x, z), w.a, w.b))
        .min()
        .unwrap_or(fx::from_int(100));
    let foot = band(cm(90), cm(250), cm(300) - nearest_wall);
    let grown = mix(green, pal::WALL_FOOT, foot * 3 / 4);
    let worn = wear(map, seed, x, z);
    if worn == 0 {
        return (grown, 0);
    }
    let earth = mix(pal::WORN, pal::WORN_DARK, value(seed + 97, x, z, 18));
    (mix(grown, earth, worn), worn)
}

impl Field {
    /// The ground for this map. The grid covers the map and the terrain's
    /// margin round it, so the view past the wall has somewhere to go.
    pub fn build(map: &Map, seed: u32) -> Field {
        let m = table::TERRAIN_MARGIN;
        let (x0, z0) = (-m, -m);
        let cols = ((map.length() + 2 * m) >> CELL_SHIFT) as usize + 1;
        let rows = ((map.width() + 2 * m) >> CELL_SHIFT) as usize + 1;
        let at = |c: usize, r: usize| (x0 + ((c as Fx) << CELL_SHIFT), z0 + ((r as Fx) << CELL_SHIFT));
        let mut heights = Vec::with_capacity(cols * rows);
        let mut colors = Vec::with_capacity(cols * rows);
        let mut worn = Vec::with_capacity(cols * rows);
        for r in 0..rows {
            for c in 0..cols {
                let (x, z) = at(c, r);
                heights.push(map.terrain.at(x, z));
                let (color, wear) = color_at(map, seed, x, z);
                // a shade under the photos: the soft light adds this back
                colors.push(pct(color, LIT));
                worn.push(wear);
            }
        }
        let h = |c: usize, r: usize| heights[r * cols + c];
        let mut normals = Vec::with_capacity(cols * rows);
        for r in 0..rows {
            for c in 0..cols {
                let (l, u) = (c.saturating_sub(1), r.saturating_sub(1));
                let (hi_c, hi_r) = ((c + 1).min(cols - 1), (r + 1).min(rows - 1));
                let run = |a: usize, b: usize| ((b - a) as Fx) << CELL_SHIFT;
                // the slope both ways, as a rise over one meter
                let sx = fx::div(h(hi_c, r) - h(l, r), run(l, hi_c));
                let sz = fx::div(h(c, hi_r) - h(c, u), run(u, hi_r));
                normals.push(unit([-sx, ONE, -sz]));
            }
        }
        Field {
            x0,
            z0,
            length: map.length(),
            width: map.width(),
            cols,
            rows,
            heights,
            normals,
            colors,
            worn,
        }
    }

    fn index(&self, c: usize, r: usize) -> usize {
        r * self.cols + c
    }

    /// The height on the triangles the ground is drawn with, so what stands
    /// on the ground stands on what the eye sees. Each cell splits from its
    /// low corner to its high corner.
    pub fn height_at(&self, x: Fx, z: Fx) -> Fx {
        let (c, u) = self.locate(x - self.x0, self.cols);
        let (r, v) = self.locate(z - self.z0, self.rows);
        let corner = |dc: usize, dr: usize| self.heights[self.index(c + dc, r + dr)] as i64;
        let (a, b, d) = (corner(0, 0), corner(1, 0), corner(1, 1));
        let e = corner(0, 1);
        let (u, v) = (u as i64, v as i64);
        let h = if u >= v {
            a + (((b - a) * u + (d - b) * v) >> 16)
        } else {
            a + (((d - e) * u + (e - a) * v) >> 16)
        };
        h as Fx
    }

    /// Which cell a distance along the grid falls in, and how far across it.
    fn locate(&self, along: Fx, count: usize) -> (usize, Fx) {
        let last = count as Fx - 2;
        let cell = (along >> CELL_SHIFT).clamp(0, last);
        let rest = (along - (cell << CELL_SHIFT)).clamp(0, CELL);
        (cell as usize, rest >> (CELL_SHIFT - 16))
    }

    /// The grid corner nearest a point.
    fn nearest(&self, x: Fx, z: Fx) -> usize {
        let c = ((x - self.x0 + CELL / 2) >> CELL_SHIFT).clamp(0, self.cols as Fx - 1) as usize;
        let r = ((z - self.z0 + CELL / 2) >> CELL_SHIFT).clamp(0, self.rows as Fx - 1) as usize;
        self.index(c, r)
    }

    /// The ground's color at any point, from the grid's nearest corner.
    pub fn color_near(&self, x: Fx, z: Fx) -> Tone {
        self.colors[self.nearest(x, z)]
    }

    /// The ground's normal at any point, from the grid's nearest corner.
    pub fn normal_near(&self, x: Fx, z: Fx) -> [Fx; 3] {
        self.normals[self.nearest(x, z)]
    }

    /// How worn the ground is at any point, from the grid's nearest corner.
    pub fn worn_near(&self, x: Fx, z: Fx) -> Fx {
        self.worn[self.nearest(x, z)]
    }

    /// Two triangles a cell, wound counter-clockwise from above.
    pub fn triangles(&self) -> Vec<u32> {
        let mut out = Vec::with_capacity((self.cols - 1) * (self.rows - 1) * 6);
        for r in 0..self.rows - 1 {
            for c in 0..self.cols - 1 {
                let i = |dc: usize, dr: usize| self.index(c + dc, r + dr) as u32;
                out.extend_from_slice(&[i(0, 0), i(1, 1), i(1, 0), i(0, 0), i(0, 1), i(1, 1)]);
            }
        }
        out
    }
}

/// The vector `v` scaled to length one. Slopes are small, so the length fits.
pub fn unit(v: [Fx; 3]) -> [Fx; 3] {
    let flat = fx::length(v[0], v[2]);
    let n = fx::length(flat, v[1]).max(1);
    v.map(|c| fx::div(c, n))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::from_int;

    fn field(seed: u64) -> (Map, Field) {
        let map = Map::generate(seed);
        let f = Field::build(&map, seed as u32);
        (map, f)
    }

    fn green_lead(c: Tone) -> i32 {
        c[1] - c[0].max(c[2])
    }

    #[test]
    fn the_distance_to_a_segment_is_right_at_its_side_and_past_its_ends() {
        let (a, b) = ((0, 0), (from_int(10), 0));
        assert_eq!(to_segment((from_int(4), from_int(3)), a, b), from_int(3));
        assert_eq!(to_segment((from_int(13), from_int(4)), a, b), from_int(5));
        assert_eq!(to_segment((-from_int(3), -from_int(4)), a, b), from_int(5));
    }

    #[test]
    fn inside_the_walls_is_green_and_outside_is_rough_hill() {
        let (map, f) = field(4);
        let (mut inside, mut green, mut outside, mut tawny) = (0, 0, 0, 0);
        for z in (0..140).step_by(7) {
            for x in (-30..290).step_by(7) {
                let (x, z) = (from_int(x), from_int(z));
                let c = f.color_near(x, z);
                if map.inside(x, z) && !map.in_fold(x, z) {
                    inside += 1;
                    green += (green_lead(c) > 0) as i32;
                } else if !map.inside(x, z) {
                    outside += 1;
                    tawny += (green_lead(c) < 0) as i32;
                }
            }
        }
        assert!(green * 100 >= inside * 95, "{green} of {inside}");
        assert!(tawny * 100 >= outside * 95, "{tawny} of {outside}");
    }

    #[test]
    fn heather_and_bracken_fade_into_the_grass() {
        // 2 m is one cell of the ground; a hard patch edge moves a channel
        // about 30/255 in that step
        let mut worst = 0;
        for z in (-400..-200).step_by(2) {
            for x in (0..200).step_by(2) {
                let (x, z) = (from_int(x), from_int(z));
                let here = moor(4, x, z);
                for (a, b) in [
                    (here, moor(4, x + from_int(2), z)),
                    (here, moor(4, x, z + from_int(2))),
                ] {
                    for ch in 0..3 {
                        worst = worst.max((a[ch] - b[ch]).abs());
                    }
                }
                // grass always shows through: never the full heather color
                assert!(
                    (0..3).any(|ch| (here[ch] - pal::HEATHER[ch]).abs() > ONE * 8 / 255),
                    "{here:?}"
                );
            }
        }
        assert!(worst < ONE * 16 / 255, "{}", worst * 255 / ONE);
    }

    #[test]
    fn a_gap_wears_the_turf_to_earth() {
        let (map, f) = field(5);
        let g = map.gaps[0];
        assert!(
            green_lead(f.color_near(g.center.0, g.center.1)) <= cm(2),
            "the middle of a gap"
        );
    }

    #[test]
    fn the_ground_mesh_covers_the_map_and_its_margin() {
        let (_, f) = field(1);
        assert_eq!(f.heights.len(), f.cols * f.rows);
        assert_eq!(f.colors.len(), f.cols * f.rows);
        // (325 + 2 * 40) / 2 m a cell, and (175 + 2 * 40) / 2 rounded down
        assert_eq!(f.cols, 203);
        assert_eq!(f.rows, 128);
        assert_eq!(f.triangles().len(), 202 * 127 * 6);
    }

    #[test]
    fn the_height_at_matches_the_triangles_at_a_cell_corner_and_middle() {
        let (map, f) = field(2);
        let (x, z) = (from_int(40), from_int(60));
        assert_eq!(f.height_at(x, z), map.terrain.at(x, z));
        // halfway along an edge of a cell: the mean of its two corners
        let (a, b) = (map.terrain.at(x, z), map.terrain.at(x + CELL, z));
        assert!((f.height_at(x + CELL / 2, z) - (a + b) / 2).abs() <= 1);
    }

    #[test]
    fn a_normal_is_a_unit_vector_leaning_downhill() {
        let (_, f) = field(3);
        let mut downhill = 0;
        for n in &f.normals {
            let len = fx::length(fx::length(n[0], n[2]), n[1]);
            assert!((len - ONE).abs() < 8, "{n:?}");
            assert!(n[1] > ONE / 2);
            downhill += (n[0] < 0) as i32;
        }
        // the map climbs toward +x, so most normals lean back toward -x
        assert!(downhill * 10 > f.normals.len() as i32 * 8, "{downhill}");
    }

    #[test]
    fn triangles_wind_counter_clockwise_from_above() {
        let (_, f) = field(1);
        let t = f.triangles();
        let corner = |i: u32| {
            let i = i as usize;
            ((i % f.cols) as i64, (i / f.cols) as i64)
        };
        for tri in t.chunks(3).take(50) {
            let (a, b, c) = (corner(tri[0]), corner(tri[1]), corner(tri[2]));
            // y is up, so a face seen from above turns from z to x
            let cross = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
            assert!(cross < 0, "{tri:?}");
        }
    }

    #[test]
    fn the_field_is_built_from_the_seed_alone() {
        assert_eq!(field(8).1, field(8).1);
        assert_ne!(field(8).1.colors, field(9).1.colors);
    }
}
