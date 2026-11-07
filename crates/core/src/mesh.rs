//! Building meshes in Q16: simple polygons, rounded where the thing is soft.
//!
//! A [`Shape`] collects triangles; each corner carries a position, a shading
//! normal, a color, which of the instance's colors it takes ([`Paint`]) and
//! the bone that moves it. [`Shape::finish`] welds equal corners into an
//! indexed mesh and works out each corner's hull normal: the average of the
//! normals meeting at that point, which the outline pass pushes along so a
//! flat-shaded model's outline has no gaps at its edges.
//!
//! Tubes and balls wind counter-clockwise from outside, so a closed model
//! can be drawn with back faces culled.

use crate::fx::{self, mul, ONE, TURN};
use crate::render::geom::{v3, Affine, V3};

/// Values per vertex: position, normal, hull normal, color, paint, bone.
pub const VERTEX_VALUES: usize = 16;

/// A color, each channel 0..=ONE.
pub type Rgb = [i32; 3];

/// From 0-255 channels.
pub const fn rgb(r: i32, g: i32, b: i32) -> Rgb {
    [
        ((r as i64 * ONE as i64) / 255) as i32,
        ((g as i64 * ONE as i64) / 255) as i32,
        ((b as i64 * ONE as i64) / 255) as i32,
    ]
}

/// A shade of a color, in percent.
pub fn shade(c: Rgb, pct: i32) -> Rgb {
    c.map(|v| (v as i64 * pct as i64 / 100).min(ONE as i64) as i32)
}

pub fn blend(a: Rgb, b: Rgb, t: i32) -> Rgb {
    [
        crate::render::geom::lerp(a[0], b[0], t),
        crate::render::geom::lerp(a[1], b[1], t),
        crate::render::geom::lerp(a[2], b[2], t),
    ]
}

/// Which colors a corner takes: shares of the instance's coat, the
/// instance's skin, and the corner's own. The corner's color multiplies the
/// result, so a coat corner's color is its shading.
pub type Paint = [i32; 3];
pub const COAT: Paint = [ONE, 0, 0];
pub const SKIN: Paint = [0, ONE, 0];
pub const OWN: Paint = [0, 0, ONE];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Vertex {
    pub pos: V3,
    pub normal: V3,
    pub hull: V3,
    pub color: Rgb,
    pub paint: Paint,
    pub bone: i32,
}

/// One ring of a tube: its middle and its half widths along the ring's own
/// side and up directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ring {
    pub at: V3,
    pub wide: i32,
    pub tall: i32,
}

/// A round ring.
pub const fn ring(at: V3, r: i32) -> Ring {
    Ring { at, wide: r, tall: r }
}

/// A coat's patches: a face's color by where it is, and the three shares the
/// pattern is given.
#[derive(Debug, Clone, Copy)]
pub struct Patch {
    pub paint: fn(V3, [i32; 3]) -> Rgb,
    pub shares: [i32; 3],
}

impl Patch {
    pub fn at(self, p: V3) -> Rgb {
        (self.paint)(p, self.shares)
    }
}

/// Triangles on their way to a mesh, with the brush's current state.
#[derive(Debug, Clone)]
pub struct Shape {
    corners: Vec<Vertex>,
    pub color: Rgb,
    pub paint: Paint,
    pub bone: i32,
    /// shade faces by their own normal (crisp facets) instead of smoothly
    pub flat: bool,
    /// a face's color by where it is: a coat's patches
    pub patch: Option<Patch>,
}

impl Default for Shape {
    fn default() -> Shape {
        Shape::new()
    }
}

/// The twelve corners and twenty faces of an icosahedron, then each face cut
/// into four `detail` times, every corner pushed out to the unit ball.
pub fn icosphere(detail: u32) -> (Vec<V3>, Vec<[usize; 3]>) {
    const PHI: i32 = 106_039;
    let raw = [
        v3(-ONE, PHI, 0),
        v3(ONE, PHI, 0),
        v3(-ONE, -PHI, 0),
        v3(ONE, -PHI, 0),
        v3(0, -ONE, PHI),
        v3(0, ONE, PHI),
        v3(0, -ONE, -PHI),
        v3(0, ONE, -PHI),
        v3(PHI, 0, -ONE),
        v3(PHI, 0, ONE),
        v3(-PHI, 0, -ONE),
        v3(-PHI, 0, ONE),
    ];
    let mut points: Vec<V3> = raw.iter().map(|p| p.norm()).collect();
    let mut faces: Vec<[usize; 3]> = vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];
    for _ in 0..detail {
        let mut seen: std::collections::BTreeMap<(usize, usize), usize> = std::collections::BTreeMap::new();
        let mut next = Vec::with_capacity(faces.len() * 4);
        for f in &faces {
            let mut mids = [0usize; 3];
            for k in 0..3 {
                let (a, b) = (f[k], f[(k + 1) % 3]);
                let key = (a.min(b), a.max(b));
                mids[k] = *seen.entry(key).or_insert_with(|| {
                    points.push(points[a].add(points[b]).norm());
                    points.len() - 1
                });
            }
            next.push([f[0], mids[0], mids[2]]);
            next.push([f[1], mids[1], mids[0]]);
            next.push([f[2], mids[2], mids[1]]);
            next.push(mids);
        }
        faces = next;
    }
    (points, faces)
}

impl Shape {
    pub fn new() -> Shape {
        Shape {
            corners: Vec::new(),
            color: [ONE; 3],
            paint: OWN,
            bone: 0,
            flat: false,
            patch: None,
        }
    }

    pub fn triangles(&self) -> usize {
        self.corners.len() / 3
    }

    /// Scales everything along x and z: a whole animal wider or longer.
    pub fn stretch(&mut self, x: i32, z: i32) {
        for c in &mut self.corners {
            c.pos = v3(mul(c.pos.x, x), c.pos.y, mul(c.pos.z, z));
        }
    }

    /// Sets the brush: its color, what it takes from the instance, its bone.
    pub fn brush(&mut self, color: Rgb, paint: Paint, bone: i32) {
        self.color = color;
        self.paint = paint;
        self.bone = bone;
    }

    fn corner(&self, pos: V3, normal: V3) -> Vertex {
        Vertex {
            pos,
            normal,
            hull: V3::ZERO,
            color: self.color,
            paint: self.paint,
            bone: self.bone,
        }
    }

    /// One triangle with a normal at each corner.
    pub fn smooth_tri(&mut self, p: [V3; 3], n: [V3; 3]) {
        for k in 0..3 {
            let c = self.corner(p[k], n[k]);
            self.corners.push(c);
        }
    }

    /// One triangle with each corner's own color and normal.
    pub fn painted_tri(&mut self, p: [V3; 3], n: [V3; 3], c: [Rgb; 3]) {
        for k in 0..3 {
            let mut v = self.corner(p[k], n[k]);
            v.color = c[k];
            self.corners.push(v);
        }
    }

    /// How many corners are drawn so far, for [`Shape::tint_from`].
    pub fn drawn(&self) -> usize {
        self.corners.len()
    }

    /// Repaints every corner from `first` on by its place: a gradient along
    /// what was just drawn.
    pub fn tint_from(&mut self, first: usize, paint: impl Fn(V3) -> Rgb) {
        for v in &mut self.corners[first..] {
            v.color = paint(v.pos);
        }
    }

    /// One flat triangle, wound as given.
    pub fn tri(&mut self, a: V3, b: V3, c: V3) {
        let n = face_normal(a, b, c);
        self.smooth_tri([a, b, c], [n; 3]);
    }

    /// A triangle wound so its face looks away from `inside`.
    fn outward(&mut self, p: [V3; 3], n: [V3; 3], inside: V3) {
        let mid = p[0].add(p[1]).add(p[2]);
        let mid = v3(mid.x / 3, mid.y / 3, mid.z / 3);
        let away = cross64_sign(p[0], p[1], p[2], mid.sub(inside));
        let (p, n) = if away >= 0 {
            (p, n)
        } else {
            ([p[0], p[2], p[1]], [n[0], n[2], n[1]])
        };
        let brush = self.color;
        if let Some(f) = self.patch {
            self.color = f.at(mid);
        }
        if self.flat {
            let f = face_normal(p[0], p[1], p[2]);
            self.smooth_tri(p, [f; 3]);
        } else {
            self.smooth_tri(p, n);
        }
        self.color = brush;
    }

    /// A triangle in the brush's color, wound so it looks away from `inside`.
    pub fn face(&mut self, a: V3, b: V3, c: V3, inside: V3) {
        let n = face_normal(a, b, c);
        self.outward([a, b, c], [n; 3], inside);
    }

    /// A ball of `radii` at `at`, turned by `turn` (a rotation), its surface
    /// `detail` subdivisions of an icosahedron (0: 20 faces, 1: 80, 2: 320).
    pub fn ball(&mut self, at: V3, radii: V3, turn: &Affine, detail: u32) {
        let (points, faces) = icosphere(detail);
        let place = turn.then(&Affine::scale(radii));
        // a stretched ball's normals: divide by the radii rather than times
        let bend = v3(
            fx::div(ONE, radii.x.max(1)),
            fx::div(ONE, radii.y.max(1)),
            fx::div(ONE, radii.z.max(1)),
        );
        let world = |p: V3| place.apply(p).add(at);
        let normal = |p: V3| turn.apply_dir(p.times(bend).norm()).norm();
        for f in faces {
            let p = [world(points[f[0]]), world(points[f[1]]), world(points[f[2]])];
            let n = [normal(points[f[0]]), normal(points[f[1]]), normal(points[f[2]])];
            self.outward(p, n, at);
        }
    }

    /// A tube through `rings` with `sides` faces, its ends capped. The rings
    /// face along the line between their neighbours, so a bent tube stays
    /// round; the side direction starts as `side` and is carried along.
    pub fn tube(&mut self, rings: &[Ring], sides: i32, side: V3) {
        let n = rings.len();
        let along = |k: usize| {
            let (a, b) = (rings[k.saturating_sub(1)].at, rings[(k + 1).min(n - 1)].at);
            b.sub(a).norm()
        };
        let mut pts: Vec<Vec<V3>> = Vec::with_capacity(n);
        let mut nor: Vec<Vec<V3>> = Vec::with_capacity(n);
        let mut u = side.norm();
        for (k, r) in rings.iter().enumerate() {
            let t = along(k);
            // the side direction, kept square to the line
            u = u.sub(t.scale(u.dot(t))).norm();
            let w = t.cross(u).norm();
            let (mut ps, mut ns) = (Vec::new(), Vec::new());
            for s in 0..sides {
                let a = TURN * s / sides;
                let (c, si) = (fx::cos(a), fx::sin(a));
                ps.push(r.at.add(u.scale(mul(c, r.wide))).add(w.scale(mul(si, r.tall))));
                let bend = u
                    .scale(mul(c, fx::div(ONE, r.wide.max(16))))
                    .add(w.scale(mul(si, fx::div(ONE, r.tall.max(16)))));
                ns.push(bend.norm());
            }
            pts.push(ps);
            nor.push(ns);
        }
        for k in 0..n - 1 {
            let mid = rings[k].at.lerp(rings[k + 1].at, ONE / 2);
            for s in 0..sides as usize {
                let e = (s + 1) % sides as usize;
                let (a, b, c, d) = (pts[k][s], pts[k][e], pts[k + 1][e], pts[k + 1][s]);
                let (na, nb, nc, nd) = (nor[k][s], nor[k][e], nor[k + 1][e], nor[k + 1][s]);
                self.outward([a, b, c], [na, nb, nc], mid);
                self.outward([a, c, d], [na, nc, nd], mid);
            }
        }
        // the ends, fans round their middles
        for (k, out) in [(0, along(0).neg()), (n - 1, along(n - 1))] {
            let hub = rings[k].at;
            for s in 0..sides as usize {
                let e = (s + 1) % sides as usize;
                self.outward([hub, pts[k][s], pts[k][e]], [out; 3], hub.sub(out));
            }
        }
    }

    /// An open cone from a ring round `base` to the point `tip`, its base
    /// sunk in whatever it grows from: a tooth, a claw.
    pub fn spike(&mut self, base: V3, tip: V3, r: i32, sides: i32, side: V3) {
        self.cone(base, tip, (r, r), sides, side);
    }

    /// A flat lock of fur: a leaf from `base` to `tip`, `wide` across `side`
    /// and `thin` the other way, lying on what it grows from.
    pub fn lock(&mut self, base: V3, tip: V3, wide: i32, thin: i32, side: V3) {
        self.cone(base, tip, (wide, thin), 6, side);
    }

    fn cone(&mut self, base: V3, tip: V3, radii: (i32, i32), sides: i32, side: V3) {
        let t = tip.sub(base).norm();
        let u = side.sub(t.scale(side.dot(t))).norm();
        let w = t.cross(u).norm();
        let pts: Vec<V3> = (0..sides)
            .map(|s| {
                let a = TURN * s / sides;
                base.add(u.scale(mul(fx::cos(a), radii.0)))
                    .add(w.scale(mul(fx::sin(a), radii.1)))
            })
            .collect();
        for s in 0..sides as usize {
            let e = (s + 1) % sides as usize;
            let (na, nb) = (pts[s].sub(base).norm(), pts[e].sub(base).norm());
            self.outward([pts[s], pts[e], tip], [na, nb, na.add(nb).add(t).norm()], base);
        }
    }

    /// Welds equal corners and works out the hull normals. Returns the
    /// vertices and the triangle list of indices into them.
    pub fn finish(mut self) -> (Vec<Vertex>, Vec<u32>) {
        let n = self.corners.len();
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by_key(|i| self.corners[*i].pos);
        let mut at = 0;
        while at < n {
            let pos = self.corners[order[at]].pos;
            let mut end = at;
            let mut sum = [0i64; 3];
            while end < n && self.corners[order[end]].pos == pos {
                let c = self.corners[order[end]].normal;
                sum = [sum[0] + c.x as i64, sum[1] + c.y as i64, sum[2] + c.z as i64];
                end += 1;
            }
            // sums of a few unit normals fit an i32; a sheet's two sides
            // cancel, and its first normal stands in
            let mut h = v3(sum[0] as i32, sum[1] as i32, sum[2] as i32).norm();
            if h == V3::ZERO {
                h = self.corners[order[at]].normal;
            }
            for i in &order[at..end] {
                self.corners[*i].hull = h;
            }
            at = end;
        }
        let mut unique = self.corners.clone();
        unique.sort();
        unique.dedup();
        let indices = self
            .corners
            .iter()
            .map(|c| unique.binary_search(c).unwrap_or(0) as u32)
            .collect();
        (unique, indices)
    }
}

pub fn face_normal(a: V3, b: V3, c: V3) -> V3 {
    // in 64 bits, then shifted down to fit: a big ground face and a tiny
    // tooth both keep their direction
    let u = [(b.x - a.x) as i64, (b.y - a.y) as i64, (b.z - a.z) as i64];
    let w = [(c.x - a.x) as i64, (c.y - a.y) as i64, (c.z - a.z) as i64];
    let n = [
        u[1] * w[2] - u[2] * w[1],
        u[2] * w[0] - u[0] * w[2],
        u[0] * w[1] - u[1] * w[0],
    ];
    let big = n.iter().map(|v| v.abs()).max().unwrap_or(0);
    let shift = (64 - big.leading_zeros() as i32 - 28).max(0);
    v3(
        (n[0] >> shift) as i32,
        (n[1] >> shift) as i32,
        (n[2] >> shift) as i32,
    )
    .norm()
}

/// The sign of (b - a) × (c - a) · d, in 64 bits and more.
fn cross64_sign(a: V3, b: V3, c: V3, d: V3) -> i64 {
    let (ux, uy, uz) = ((b.x - a.x) as i128, (b.y - a.y) as i128, (b.z - a.z) as i128);
    let (wx, wy, wz) = ((c.x - a.x) as i128, (c.y - a.y) as i128, (c.z - a.z) as i128);
    let cx = uy * wz - uz * wy;
    let cy = uz * wx - ux * wz;
    let cz = ux * wy - uy * wx;
    (cx * d.x as i128 + cy * d.y as i128 + cz * d.z as i128).signum() as i64
}

/// The vertex values of a list of vertices, as the page reads them. The
/// bone goes out as its first column in the pose texture.
pub fn write_vertices(vs: &[Vertex], texels_per_bone: i32, out: &mut Vec<i32>) {
    for v in vs {
        out.extend_from_slice(&[
            v.pos.x,
            v.pos.y,
            v.pos.z,
            v.normal.x,
            v.normal.y,
            v.normal.z,
            v.hull.x,
            v.hull.y,
            v.hull.z,
            v.color[0],
            v.color[1],
            v.color[2],
            v.paint[0],
            v.paint[1],
            v.paint[2],
            v.bone * texels_per_bone * ONE,
        ]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::geom::{cm, mm};

    fn inside_out(vs: &[Vertex], ix: &[u32], middle: V3) -> usize {
        ix.chunks(3)
            .filter(|t| {
                let (a, b, c) = (vs[t[0] as usize], vs[t[1] as usize], vs[t[2] as usize]);
                let mid = a.pos.add(b.pos).add(c.pos);
                let mid = v3(mid.x / 3, mid.y / 3, mid.z / 3);
                cross64_sign(a.pos, b.pos, c.pos, mid.sub(middle)) < 0
            })
            .count()
    }

    #[test]
    fn an_icosphere_has_twenty_faces_times_four_per_cut() {
        assert_eq!(icosphere(0).1.len(), 20);
        assert_eq!(icosphere(1).1.len(), 80);
        // closed: V - E + F = 2, each face with three edges shared by two
        let (p, f) = icosphere(2);
        assert_eq!(f.len(), 320);
        assert_eq!(p.len() as i64 - (f.len() as i64 * 3 / 2) + f.len() as i64, 2);
    }

    #[test]
    fn a_ball_faces_outward_and_welds_its_corners() {
        let mut s = Shape::new();
        s.ball(v3(0, ONE, 0), v3(cm(30), cm(20), cm(40)), &Affine::IDENTITY, 1);
        let (vs, ix) = s.finish();
        assert_eq!(ix.len(), 80 * 3);
        assert_eq!(vs.len(), 42, "the 42 corners of the once-cut icosahedron");
        assert_eq!(inside_out(&vs, &ix, v3(0, ONE, 0)), 0);
    }

    #[test]
    fn a_tapered_tube_faces_outward_with_closed_ends() {
        let mut s = Shape::new();
        s.flat = true;
        let rings = [
            ring(v3(0, cm(40), 0), cm(5)),
            ring(v3(cm(2), cm(20), 0), cm(4)),
            ring(v3(cm(2), 0, 0), cm(3)),
        ];
        s.tube(&rings, 6, v3(ONE, 0, 0));
        // 6 sides × 2 sections × 2 triangles, and 6 in each end's fan
        assert_eq!(s.triangles(), 6 * 2 * 2 + 12);
        let (vs, ix) = s.finish();
        assert_eq!(inside_out(&vs, &ix, v3(cm(1), cm(20), 0)), 0);
    }

    #[test]
    fn hull_normals_are_shared_where_flat_faces_meet() {
        let mut s = Shape::new();
        s.flat = true;
        s.tube(
            &[ring(v3(0, 0, 0), cm(10)), ring(v3(0, ONE, 0), cm(10))],
            6,
            v3(ONE, 0, 0),
        );
        let (vs, _) = s.finish();
        // every corner at one point has one hull normal, whatever its face
        for a in &vs {
            for b in &vs {
                if a.pos == b.pos {
                    assert_eq!(a.hull, b.hull);
                }
            }
            assert!((a.hull.len() - ONE).abs() < 8);
        }
        // and a side corner's hull leans out, not along either face alone
        let top = vs.iter().find(|v| v.pos.y == ONE && v.pos.x > cm(9)).unwrap();
        assert!(top.hull.y > mm(100) && top.hull.x > mm(100), "{:?}", top.hull);
    }

    #[test]
    fn paint_multiplies_a_coat_corner_and_a_shade_stays_in_range() {
        assert_eq!(shade([ONE; 3], 50), [ONE / 2; 3]);
        assert_eq!(shade([ONE; 3], 150), [ONE; 3]);
        assert_eq!(rgb(255, 0, 0), [ONE, 0, 0]);
    }
}
