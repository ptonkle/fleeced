//! The Sheepdog: a border collie the player likes being, as soft as the Cloud
//! sheep beside it. Every part is one smooth closed skin; [`lay`] raises soft
//! lumps out of it, melted together as a sheep's wool is, long where fur hangs
//! (the ruff, the bib, the belly, the backs of the legs, the tail) and barely
//! there on the back. A crease goes a little darker and the black catches a
//! blue-gray sheen from above. Black over the back and head, a white blaze and
//! muzzle, a shaggy white ruff and shirt front, white socks, a plumed tail with
//! a white tip. The fringes carry soft tufts ([`fray`]), the same hand as the
//! Wolf's pelt but rounder and fewer. From the play camera it reads as a glossy black back ringed by
//! a white ruff, unlike any sheep or the Wolf.
//!
//! The pattern is a mark: a signed field over the body, positive on white.
//! [`lay`] cuts each face along the field's zero line, so an edge runs clean
//! across the facets, and the lumps wobble the field a little, so black meets
//! white in soft scallops.

use crate::fx::{self, div, mul, ONE};
use crate::mesh::{blend, rgb, Rgb, Ring, Shape, OWN};
use crate::render::geom::{cm, deg, lerp, mm, smooth, v3, Affine, V3};

use super::rig::{
    self, leg_bone, Leg, Rig, Skeleton, BODY, CHEST, EAR_LEFT, EAR_RIGHT, HEAD, JAW, NECK, TAIL,
};
use super::sheep::noise;

/// A crease in the black, the black itself, the sheen on a crest, and the
/// sun-bleached brown it drifts to.
const DEEP: Rgb = rgb(24, 23, 29);
const BLACK: Rgb = rgb(50, 48, 58);
const SHEEN: Rgb = rgb(128, 130, 150);
const RUST: Rgb = rgb(112, 80, 58);
/// A crease in the white, the white itself, and its warm drift.
const SHADOW: Rgb = rgb(186, 186, 198);
const WHITE: Rgb = rgb(248, 244, 236);
const CREAM: Rgb = rgb(238, 224, 198);
const NOSE: Rgb = rgb(22, 20, 22);
const IRIS: Rgb = rgb(122, 76, 34);
const PUPIL: Rgb = rgb(14, 12, 12);
const GLINT: Rgb = rgb(255, 255, 255);
const MOUTH: Rgb = rgb(92, 40, 48);
const TONGUE: Rgb = rgb(226, 118, 128);
const EAR_IN: Rgb = rgb(112, 78, 80);

/// Faces round the trunk and the ruff.
const ROUND: i32 = 36;
/// Faces round a leg, and round the tail.
const LIMB: i32 = 12;
/// Faces round the snout and the ears.
const SOFT: i32 = 18;
/// Steps between two rings: the trunk, then a limb.
const FINE: i32 = 3;
const FINER: i32 = 3;
/// How wide a meld between two lumps is.
const MELT: i32 = ONE / 3;

const fn side() -> V3 {
    v3(ONE, 0, 0)
}

/// A new piece of the coat on `bone`, to be laid by [`lay`].
fn piece(bone: i32) -> Shape {
    let mut part = Shape::new();
    part.brush(BLACK, OWN, bone);
    part
}

/// How the fur lies on a part: which way the hair runs at a point, how far it
/// stands there (given the point and its outward normal), and how big a lump
/// is, in meters, half long and half wide, on a grid of `cell`.
struct Nap<'a> {
    flow: &'a dyn Fn(V3) -> V3,
    rise: &'a dyn Fn(V3, V3) -> i32,
    long: i32,
    wide: i32,
    cell: i32,
    /// how far the lumps push a pattern edge
    edge: i32,
}

/// The softer of two heights: rounds the join where lumps meet.
fn soft_max(a: i32, b: i32) -> i32 {
    let h = (MELT - (a - b).abs()).max(0);
    a.max(b) + mul(mul(h, h), div(ONE, MELT)) / 4
}

/// The fur at `p`, 0 in a crease to about `ONE` on a lump: the lumps nearby,
/// each a dome drawn out along the hair, melted together.
fn lump(p: V3, nap: &Nap) -> i32 {
    let flow = (nap.flow)(p);
    let cell = nap.cell;
    let scale = div(ONE, nap.wide);
    let stretch = div(nap.wide, nap.long);
    let at = |v: i32| v.div_euclid(cell);
    let (cx, cy, cz) = (at(p.x), at(p.y), at(p.z));
    let mut best = 0;
    for i in cx - 1..=cx + 1 {
        for j in cy - 1..=cy + 1 {
            for k in cz - 1..=cz + 1 {
                let id = i.wrapping_mul(73) ^ j.wrapping_mul(179) ^ k.wrapping_mul(283);
                let spot = |n: i32, wob: i32| n * cell + cell / 2 + mul(noise(wob), cell * 2 / 5);
                let c = v3(spot(i, id), spot(j, id + 1), spot(k, id + 2));
                let q = p.sub(c).scale(scale);
                let along = q.dot(flow);
                let t = mul(along, stretch);
                let across = q.dot(q) - mul(along, along) + mul(t, t);
                if across >= ONE {
                    continue;
                }
                let dome = smooth(0, ONE, ONE - across);
                best = soft_max(best, mul(dome, ONE * 3 / 4 + noise(id + 3) / 4));
            }
        }
    }
    best.min(ONE)
}

/// One corner of the fur.
#[derive(Clone, Copy)]
struct Hair {
    pos: V3,
    normal: V3,
    /// the mark at this corner: above zero is white
    mark: i32,
    /// 0 in a crease, `ONE` on a lump
    crest: i32,
}

/// The coat's color at a corner: a crease goes a little dark, the black
/// catches the light from above, and a slow drift over the body warms it.
fn tone(white: bool, h: &Hair) -> Rgb {
    let p = h.pos;
    let drift = (fx::sin(p.z * 2 + p.x * 3) + fx::sin(p.y * 3 - p.z)) / 2;
    if white {
        let c = blend(SHADOW, WHITE, smooth(0, ONE * 7 / 10, h.crest));
        blend(c, CREAM, drift.max(0) / 3)
    } else {
        let lit = smooth(0, ONE * 7 / 10, h.crest);
        let c = blend(DEEP, BLACK, lit);
        let c = blend(c, RUST, drift.max(0) / 3);
        let high = smooth(ONE / 10, ONE * 8 / 10, h.normal.y);
        blend(c, SHEEN, mul(high, ONE / 7 + lit / 7))
    }
}

/// Moves `part` onto `s` with its fur laid: every corner raised by the lumps
/// along its normal, its light taken from the new surface, and every face cut
/// where `mark` crosses it.
fn lay(s: &mut Shape, part: Shape, nap: &Nap, mark: impl Fn(V3) -> i32) {
    let (vs, ix) = part.finish();
    let crests: Vec<i32> = vs
        .iter()
        .map(|v| if nap.cell > 0 { lump(v.pos, nap) } else { ONE })
        .collect();
    let moved: Vec<V3> = vs
        .iter()
        .zip(&crests)
        .map(|(v, c)| v.pos.add(v.normal.scale(mul((nap.rise)(v.pos, v.normal), *c))))
        .collect();
    let faces = ix.as_chunks::<3>().0;
    let mut normals = vec![V3::ZERO; vs.len()];
    let wide = fx::from_int(8);
    for f in faces {
        let k = f.map(|c| c as usize);
        let (a, b, c) = (moved[k[0]], moved[k[1]], moved[k[2]]);
        let n = b.sub(a).scale(wide).cross(c.sub(a).scale(wide));
        for c in k {
            normals[c] = normals[c].add(n);
        }
    }
    let hairs: Vec<Hair> = (0..vs.len())
        .map(|k| Hair {
            pos: moved[k],
            normal: normals[k].norm().add(vs[k].normal.scale(ONE * 4 / 5)).norm(),
            mark: mark(vs[k].pos) + mul(crests[k] - ONE / 2, nap.edge),
            crest: crests[k],
        })
        .collect();
    for f in faces {
        s.bone = vs[f[0] as usize].bone;
        cut_face(s, f.map(|c| hairs[c as usize]));
    }
}

/// Grows soft tufts out of the fur of `part`, one wherever the faces gone by add
/// up to `gap` squared, as long as `long` says there (none at zero): the same
/// shingles as the Wolf's pelt, but rounder, lying closer, and only some of
/// them, so the coat stays soft and the two animals share one hand.
fn fray(
    s: &mut Shape,
    part: &Shape,
    nap: &Nap,
    gap: i32,
    long: &dyn Fn(V3, V3) -> i32,
    mark: impl Fn(V3) -> i32,
) {
    let (vs, ix) = part.clone().finish();
    // in eighths: the area of a small face would lose its digits
    let eight = fx::from_int(8);
    let room = mul(gap * 8, gap * 8);
    let (mut area, mut grown) = (0, 0);
    for f in ix.as_chunks::<3>().0 {
        let v = f.map(|c| vs[c as usize]);
        let wide = v[1].pos.sub(v[0].pos).scale(eight);
        area += wide.cross(v[2].pos.sub(v[0].pos).scale(eight)).len() / 2;
        while area >= room {
            area -= room;
            grown += 1;
            let (a, b) = (noise(grown * 2) / 2 + ONE / 2, noise(grown * 2 + 1) / 2 + ONE / 2);
            let (a, b) = if a + b > ONE { (ONE - a, ONE - b) } else { (a, b) };
            let c = ONE - a - b;
            let at = v[0].pos.scale(a).add(v[1].pos.scale(b)).add(v[2].pos.scale(c));
            let n = v[0]
                .normal
                .scale(a)
                .add(v[1].normal.scale(b))
                .add(v[2].normal.scale(c))
                .norm();
            let size = long(at, n);
            if size > 0 {
                s.bone = v[0].bone;
                let root = at.add(n.scale(mul((nap.rise)(at, n), ONE * 6 / 10)));
                tuft(s, (nap.flow)(at), root, n, size, mark(at) > 0, grown);
            }
        }
    }
}

/// One tuft: a blunt leaf rooted in the fur and lying down it, with a ridge
/// down its middle, its light kept close to the skin's.
fn tuft(s: &mut Shape, flow: V3, at: V3, n: V3, long: i32, white: bool, seed: i32) {
    // hair that would run straight out of the skin falls down it instead
    let flow = flow.add(v3(0, -flow.dot(n).abs(), 0));
    let t = flow.sub(n.scale(flow.dot(n)));
    if t.len() < ONE / 4 {
        return;
    }
    let t = t.norm();
    let b = n.cross(t).norm();
    let long = mul(long, ONE * 9 / 10 + noise(seed + 500) / 5);
    let wide = long * 42 / 100;
    let lean = noise(seed + 700) / 3;
    let peak = ONE * 3 / 4 + noise(seed + 900) / 4;
    let point = |ahead: i32, across: i32, up: i32| {
        at.add(t.scale(mul(long, ahead)))
            .add(b.scale(mul(wide, across)))
            .add(n.scale(mul(long, up)))
    };
    let corner = |pos: V3, normal: V3, crest: i32| Hair {
        pos,
        normal: normal.norm(),
        mark: 0,
        crest,
    };
    let (flank, fall) = (b.scale(ONE / 3), t.scale(ONE / 4));
    let root = corner(point(-ONE / 6, 0, -ONE / 8), n.sub(fall), ONE / 2);
    let left = corner(
        point(ONE / 2, -ONE, ONE / 16),
        n.sub(flank),
        mul(peak, ONE * 7 / 10),
    );
    let right = corner(
        point(ONE / 2, ONE, ONE / 16),
        n.add(flank),
        mul(peak, ONE * 7 / 10),
    );
    let ridge = corner(point(ONE / 2, 0, ONE / 8), n, peak);
    let tip = corner(point(ONE, lean, ONE / 10), n.add(fall), peak);
    let eight = fx::from_int(8);
    for (a, b, c) in [
        (root, right, ridge),
        (root, ridge, left),
        (left, ridge, tip),
        (ridge, right, tip),
    ] {
        let wound = b.pos.sub(a.pos).scale(eight).cross(c.pos.sub(a.pos).scale(eight));
        if wound.dot(n) > 0 {
            paint(s, white, a, b, c);
        } else {
            paint(s, white, a, c, b);
        }
    }
}

/// A coat with no lumps: for what is too small to carry fur.
fn coat(s: &mut Shape, part: Shape, mark: impl Fn(V3) -> i32) {
    let nap = Nap {
        flow: &|_| v3(0, 0, -ONE),
        rise: &|_, _| 0,
        long: ONE,
        wide: ONE,
        cell: 0,
        edge: 0,
    };
    lay(s, part, &nap, mark);
}

fn paint(s: &mut Shape, white: bool, a: Hair, b: Hair, c: Hair) {
    s.painted_tri(
        [a.pos, b.pos, c.pos],
        [a.normal, b.normal, c.normal],
        [tone(white, &a), tone(white, &b), tone(white, &c)],
    );
}

fn cut_face(s: &mut Shape, v: [Hair; 3]) {
    let white = v.map(|h| h.mark > 0);
    if white[0] == white[1] && white[1] == white[2] {
        paint(s, white[0], v[0], v[1], v[2]);
        return;
    }
    // the corner alone on its side of the line, then the two after it
    let lone = (0..3)
        .find(|k| white[*k] != white[(k + 1) % 3] && white[*k] != white[(k + 2) % 3])
        .unwrap_or(0);
    let (b, c) = ((lone + 1) % 3, (lone + 2) % 3);
    let (ab, ac) = (crossing(&v, lone, b), crossing(&v, lone, c));
    paint(s, white[lone], v[lone], ab, ac);
    paint(s, white[b], ab, v[b], v[c]);
    paint(s, white[b], ab, v[c], ac);
}

/// Where the mark is zero on the edge `a` to `b`. Worked from the same end
/// whichever face asks, so two faces sharing the edge meet exactly.
fn crossing(v: &[Hair; 3], a: usize, b: usize) -> Hair {
    let (a, b) = if v[a].pos <= v[b].pos { (a, b) } else { (b, a) };
    let (a, b) = (v[a], v[b]);
    let t = div(a.mark, a.mark - b.mark);
    Hair {
        pos: a.pos.lerp(b.pos, t),
        normal: a.normal.lerp(b.normal, t).norm(),
        mark: 0,
        crest: lerp(a.crest, b.crest, t),
    }
}

/// `rings` with `steps` more between each two, on a smooth curve through
/// them: a soft outline, and corners close enough to carry fur.
fn fine(rings: &[Ring], steps: i32) -> Vec<Ring> {
    let last = rings.len() - 1;
    let curve = |a: i32, b: i32, c: i32, d: i32, t: i32| {
        let t2 = mul(t, t);
        let t3 = mul(t2, t);
        (2 * b + mul(c - a, t) + mul(2 * a - 5 * b + 4 * c - d, t2) + mul(3 * b - a - 3 * c + d, t3)) / 2
    };
    let mut out = Vec::new();
    for k in 0..last {
        let (p, q) = (rings[k.saturating_sub(1)], rings[k]);
        let (r, n) = (rings[k + 1], rings[(k + 2).min(last)]);
        for step in 0..steps {
            let t = step * ONE / steps;
            let at = |f: fn(&Ring) -> i32| curve(f(&p), f(&q), f(&r), f(&n), t);
            out.push(Ring {
                at: v3(at(|o| o.at.x), at(|o| o.at.y), at(|o| o.at.z)),
                wide: at(|o| o.wide).max(mm(3)),
                tall: at(|o| o.tall).max(mm(3)),
            });
        }
    }
    out.push(rings[last]);
    out
}

/// Oval rings along the body, in mm: (z, y, half width, half height).
fn rings(list: &[(i32, i32, i32, i32)]) -> Vec<Ring> {
    list.iter()
        .map(|(z, y, w, t)| Ring {
            at: v3(0, mm(*y), mm(*z)),
            wide: mm(*w),
            tall: mm(*t),
        })
        .collect()
}

fn oval(at: V3, wide: i32, tall: i32) -> Ring {
    Ring {
        at,
        wide: mm(wide),
        tall: mm(tall),
    }
}

/// White under the body: the belly, and a shirt front from the collar down
/// to behind the forelegs.
fn body_mark(p: V3) -> i32 {
    let belly = (mm(362) - p.y).min(p.z + mm(150));
    let shirt = p.z - mm(176) + mul(mm(470) - p.y, ONE * 3 / 5);
    belly.max(shirt)
}

/// White forelegs out of the white shirt, and white socks past the hock.
fn fore_mark(_: V3) -> i32 {
    ONE
}

fn hind_mark(p: V3) -> i32 {
    mm(186) - p.y
}

/// Hair that runs back along the body and fans off the spine down the flanks.
fn body_flow(p: V3) -> V3 {
    let out = div(p.x, mm(120)).clamp(-ONE, ONE);
    v3(
        mul(out, ONE * 2 / 5),
        -ONE * 3 / 10 - mul(out.abs(), ONE / 3),
        -ONE,
    )
    .norm()
}

/// The coat barely stands on the back and hangs in a fringe under the belly.
fn body_rise(_: V3, n: V3) -> i32 {
    mm(12) + mul(mm(30), smooth(ONE / 4, ONE, -n.y))
}

/// Short tufts on the flanks and a longer fringe under the belly; none under
/// the ruff, or where the two halves of the trunk overlap.
fn body_tuft(p: V3, n: V3) -> i32 {
    if p.z > mm(170) || (p.z > -mm(60) && p.z < mm(10)) {
        return 0;
    }
    mm(48) + mul(mm(34), smooth(ONE / 4, ONE, -n.y))
}

/// The trunk, in two halves that overlap at the waist: a deep chest, a tucked
/// waist and a round rump. The back is short and strong: high at the withers,
/// falling a little to the croup, with no flat side anywhere.
fn trunk(s: &mut Shape) {
    let nap = Nap {
        flow: &body_flow,
        rise: &body_rise,
        long: mm(84),
        wide: mm(52),
        cell: mm(54),
        edge: mm(26),
    };
    // one profile cut in two: where the bones overlap the skins lie exactly
    // on each other, so no seam crosses the back
    let line = fine(
        &rings(&[
            (-404, 462, 7, 7),
            (-398, 462, 30, 28),
            (-382, 464, 80, 66),
            (-340, 462, 114, 94),
            (-280, 462, 124, 100),
            (-205, 464, 114, 98),
            (-135, 468, 108, 94),
            (-60, 464, 116, 100),
            (10, 446, 128, 122),
            (100, 432, 142, 134),
            (180, 432, 138, 130),
            (240, 448, 114, 116),
            (276, 456, 60, 60),
            (282, 458, 7, 7),
        ]),
        FINE,
    );
    let step = FINE as usize;
    let mut rear = piece(BODY);
    rear.tube(&line[..=8 * step], ROUND, side());
    fray(s, &rear, &nap, mm(32), &body_tuft, body_mark);
    lay(s, rear, &nap, body_mark);
    let mut front = piece(CHEST);
    front.tube(&line[7 * step..], ROUND, side());
    fray(s, &front, &nap, mm(32), &body_tuft, body_mark);
    lay(s, front, &nap, body_mark);
}

/// The ruff: one deep shaggy white mane round the neck, its rear edge lying
/// over the shoulders, and the long bib below it.
fn ruff(s: &mut Shape) {
    let flow = |p: V3| {
        let out = div(p.x, mm(100)).clamp(-ONE, ONE);
        v3(mul(out, ONE * 2 / 5), -ONE * 7 / 10, -ONE * 3 / 5).norm()
    };
    let rise = |_: V3, n: V3| mm(32) + mul(mm(8), n.y.abs());
    let nap = Nap {
        flow: &flow,
        rise: &rise,
        long: mm(66),
        wide: mm(36),
        cell: mm(34),
        edge: 0,
    };
    let mut mane = piece(NECK);
    mane.tube(
        &fine(
            &rings(&[
                (130, 474, 8, 8),
                (136, 478, 44, 44),
                (156, 492, 92, 92),
                (204, 524, 100, 102),
                (258, 572, 88, 88),
                (290, 618, 68, 64),
                (302, 640, 30, 28),
                (306, 648, 7, 7),
            ]),
            FINE,
        ),
        ROUND,
        side(),
    );
    fray(s, &mane, &nap, mm(26), &|_, _| mm(68), |_| ONE);
    lay(s, mane, &nap, |_| ONE);

    let hang = |_: V3| v3(0, -ONE, -ONE / 8).norm();
    let long = |_: V3, _: V3| mm(34);
    let nap = Nap {
        flow: &hang,
        rise: &long,
        long: mm(66),
        wide: mm(34),
        cell: mm(34),
        edge: 0,
    };
    let mut bib = piece(CHEST);
    bib.ball(
        v3(0, mm(404), mm(272)),
        v3(mm(60), mm(92), mm(46)),
        &Affine::rot_x(deg(14)),
        3,
    );
    fray(s, &bib, &nap, mm(26), &|_, _| mm(74), |_| ONE);
    lay(s, bib, &nap, |_| ONE);
}

/// How a leg is shaped: half widths side to side and fore to aft, mm, at the
/// hip, the knee and the ankle; and how long the feathering hangs off the
/// back of it, between which heights.
struct LegLook {
    hip: (i32, i32),
    knee: (i32, i32),
    ankle: (i32, i32),
    feather: i32,
    from: i32,
    to: i32,
    /// how far the top of the leg runs up the body, mm
    haunch: i32,
}

fn at3(p: (i32, i32), x: i32) -> V3 {
    v3(x, p.1, p.0)
}

/// A sturdy tapered leg with a neat rounded paw, its hair running down and
/// feathering off the back.
fn leg(s: &mut Shape, l: &Leg, index: usize, look: &LegLook, mark: fn(V3) -> i32) {
    let x = if index.is_multiple_of(2) { l.x } else { -l.x };
    // the top of the leg leans in, so the thigh melts into the body
    let inner = x * 3 / 4;
    let top = (
        l.hip.0 + (l.knee.0 - l.hip.0) / 4,
        l.hip.1 + (l.knee.1 - l.hip.1) / 4,
    );
    let flow = |_: V3| v3(0, -ONE, -ONE / 3).norm();
    let rise = |p: V3, n: V3| {
        let hang = mul(
            smooth(look.to - mm(60), look.to, p.y),
            ONE - smooth(look.from, look.from + mm(40), p.y),
        );
        let aft = smooth(0, ONE * 8 / 10, -n.z);
        mm(4) + mul(mul(look.feather, hang), aft)
    };
    let nap = Nap {
        flow: &flow,
        rise: &rise,
        long: mm(40),
        wide: mm(24),
        cell: mm(24),
        edge: mm(14),
    };
    // past a joint each bone runs on, narrowing inside its neighbor, so no
    // gap opens when the leg folds
    let run_on = |p: (i32, i32), q: (i32, i32)| (p.0 + (q.0 - p.0) / 4, p.1 + (q.1 - p.1) / 4);
    let inside = |p: (i32, i32), size: (i32, i32), x: i32| oval(at3(p, x), size.0 * 9 / 10, size.1 * 9 / 10);
    let haunch = (l.hip.0, l.hip.1 + mm(look.haunch));
    let mut upper = piece(leg_bone(index, 0));
    upper.tube(
        &fine(
            &[
                oval(at3(haunch, inner), look.hip.0 * 5 / 10, look.hip.1 * 6 / 10),
                oval(at3(top, inner), look.hip.0, look.hip.1),
                oval(at3(l.knee, x), look.knee.0, look.knee.1),
                inside(run_on(l.knee, l.ankle), look.knee, x),
            ],
            FINER,
        ),
        LIMB,
        side(),
    );
    let fringe = |p: V3, n: V3| {
        let aft = n.z < -ONE / 4 && p.y > look.to && p.y < look.from + mm(40);
        if aft {
            mm(50)
        } else {
            0
        }
    };
    fray(s, &upper, &nap, mm(20), &fringe, mark);
    lay(s, upper, &nap, mark);

    let mut lower = piece(leg_bone(index, 1));
    lower.tube(
        &fine(
            &[
                inside(run_on(l.knee, l.hip), look.knee, x),
                oval(at3(l.knee, x), look.knee.0, look.knee.1),
                oval(at3(l.ankle, x), look.ankle.0, look.ankle.1),
                inside(run_on(l.ankle, l.foot), look.ankle, x),
            ],
            FINER,
        ),
        LIMB,
        side(),
    );
    fray(s, &lower, &nap, mm(20), &fringe, mark);
    lay(s, lower, &nap, mark);

    let mut foot = piece(leg_bone(index, 2));
    let heel = (l.foot.0, l.foot.1 + mm(26));
    foot.tube(
        &fine(
            &[
                oval(
                    at3(run_on(l.ankle, l.foot), x),
                    look.ankle.0 * 7 / 10,
                    look.ankle.1 * 7 / 10,
                ),
                oval(at3(l.ankle, x), look.ankle.0 * 8 / 10, look.ankle.1 * 8 / 10),
                oval(at3(heel, x), look.ankle.0 + 3, look.ankle.1 + 3),
            ],
            FINER,
        ),
        LIMB,
        side(),
    );
    let paw = (l.foot.0 + mm(12), l.foot.1 + mm(24));
    foot.ball(
        at3(paw, x),
        v3(mm(look.ankle.0 + 6), mm(23), mm(look.ankle.1 + 10)),
        &Affine::IDENTITY,
        2,
    );
    // three toes at the front of the paw
    for toe in [-1i32, 0, 1] {
        let ahead = l.foot.0 + mm(42 - toe.abs() * 6);
        foot.ball(
            v3(x + mm(toe * 17), l.foot.1 + mm(15), ahead),
            v3(mm(11), mm(14), mm(15)),
            &Affine::IDENTITY,
            1,
        );
    }
    lay(s, foot, &nap, mark);
}

fn legs(s: &mut Shape, sk: &Skeleton) {
    let fore = LegLook {
        hip: (44, 58),
        knee: (34, 38),
        ankle: (26, 28),
        feather: mm(34),
        from: cm(30),
        to: cm(14),
        haunch: 0,
    };
    let hind = LegLook {
        hip: (50, 80),
        knee: (34, 42),
        ankle: (22, 26),
        feather: mm(46),
        from: cm(40),
        to: cm(20),
        haunch: 70,
    };
    for index in 0..4 {
        if index < 2 {
            leg(s, &sk.fore, index, &fore, fore_mark);
        } else {
            leg(s, &sk.hind, index, &hind, hind_mark);
        }
    }
}

/// A full feathered tail hanging in a low curve, its tip swept up and white.
/// The hair runs down the tail and hangs off its underside in a long fringe.
fn tail(s: &mut Shape, sk: &Skeleton) {
    let [a, b, c] = sk.tail;
    let back = v3(0, 0, -mm(10));
    let mid = |p: V3, q: V3| p.lerp(q, ONE / 2);
    let up = c.add(v3(0, -mm(4), -mm(44)));
    let tip = c.add(v3(0, mm(44), -mm(96)));
    let start = b.lerp(c, ONE * 4 / 5);
    let along = v3(0, -ONE * 4 / 5, -ONE * 3 / 5);
    let mark = move |p: V3| p.sub(start).dot(along);
    let flow = |_: V3| v3(0, -ONE * 3 / 4, -ONE * 2 / 3).norm();
    let rise = move |p: V3, n: V3| {
        let plume = mm(12) + mul(mm(26), smooth(0, ONE, (-n.y - n.z) / 2 + ONE / 8));
        mul(plume, smooth(0, mm(130), p.dist(tip)))
    };
    let nap = Nap {
        flow: &flow,
        rise: &rise,
        long: mm(56),
        wide: mm(30),
        cell: mm(30),
        edge: mm(20),
    };

    let line = [
        oval(a.add(v3(0, mm(14), mm(16))), 22, 22),
        oval(a, 30, 30),
        oval(mid(a, b).add(back), 30, 34),
        oval(b.add(back.scale(ONE * 2)), 30, 38),
        oval(mid(b, c).add(back.scale(ONE * 2)), 30, 40),
        oval(c.add(back), 28, 36),
        oval(up, 22, 26),
        oval(tip.add(v3(0, -mm(18), mm(20))), 12, 13),
        oval(tip, 4, 4),
    ];
    for (k, bone) in TAIL.iter().enumerate() {
        let mut part = piece(*bone);
        // past a joint the tube narrows inside its neighbor
        let mut own = [
            line[k * 2],
            line[k * 2 + 1],
            line[k * 2 + 2],
            line[k * 2 + 3],
            line[k * 2 + 4],
        ];
        for end in [0, 4] {
            if (end == 0 && k > 0) || (end == 4 && k < 2) {
                own[end].wide = own[end].wide * 7 / 10;
                own[end].tall = own[end].tall * 7 / 10;
            }
        }
        part.tube(&fine(&own, FINER), LIMB, side());
        let plume = |p: V3, _: V3| mul(mm(62), smooth(0, mm(90), p.dist(tip)));
        fray(s, &part, &nap, mm(22), &plume, mark);
        lay(s, part, &nap, mark);
    }
}

fn head_frame(down: i32) -> (V3, V3) {
    (
        v3(0, -fx::sin(down), fx::cos(down)),
        v3(0, fx::cos(down), fx::sin(down)),
    )
}

/// A friendly head: a round skull, a clear stop, a short soft muzzle, a white
/// blaze, soft cheeks, big dark eyes that catch the light, ears that tip over,
/// and a lolling tongue.
fn head(s: &mut Shape, sk: &Skeleton) {
    let (fwd, up) = head_frame(deg(8));
    let o = sk.head;
    let h = move |along: i32, high: i32, x: i32| {
        o.add(fwd.scale(mm(along)))
            .add(up.scale(mm(high)))
            .add(side().scale(mm(x)))
    };
    // the blaze narrows between the eyes and spreads over the muzzle; the
    // muzzle and the chin are white, the cheeks black
    let mark = move |p: V3| {
        let d = p.sub(o);
        let (along, high, x) = (d.dot(fwd), d.dot(up), d.x.abs());
        let blaze = (mm(19) + (along - mm(30)).max(0) / 4 - x).min((along + mm(62)) / 3 - x);
        let muzzle = along - mm(70) - x / 2;
        let chin = -high - mm(40);
        blaze.max(muzzle).max(chin)
    };
    let flow = |_: V3| fwd.neg();
    let rise = |_: V3, _: V3| mm(5);
    let nap = Nap {
        flow: &flow,
        rise: &rise,
        long: mm(34),
        wide: mm(24),
        cell: mm(24),
        edge: mm(6),
    };
    let mut part = piece(HEAD);
    part.ball(h(0, 34, 0), v3(mm(92), mm(84), mm(98)), &Affine::IDENTITY, 3);
    for x in [-1, 1] {
        // a soft cheek behind the eye
        part.ball(
            h(-12, -6, x * 46),
            v3(mm(44), mm(48), mm(50)),
            &Affine::IDENTITY,
            2,
        );
    }
    // the same soft tufts as the coat, kept off the eyes
    let clear_of_eyes = move |p: V3| [-1, 1].iter().all(|x| p.sub(h(70, 48, x * 58)).len() > mm(38));
    // and off the blaze's edge, so the black and the white stay clean
    let skull_tuft = move |p: V3, _: V3| {
        if clear_of_eyes(p) && mark(p).abs() > mm(26) {
            mm(50)
        } else {
            0
        }
    };
    fray(s, &part, &nap, mm(30), &skull_tuft, mark);
    lay(s, part, &nap, mark);

    let mut snout = piece(HEAD);
    snout.tube(
        &fine(
            &[
                oval(h(30, 0, 0), 30, 30),
                oval(h(56, -4, 0), 62, 56),
                oval(h(96, -14, 0), 50, 42),
                oval(h(136, -19, 0), 44, 37),
                oval(h(160, -23, 0), 36, 30),
                oval(h(172, -25, 0), 18, 15),
            ],
            FINER,
        ),
        SOFT,
        side(),
    );
    // shorter on the muzzle, and none on the nose
    let muzzle_tuft = move |p: V3, _: V3| {
        if clear_of_eyes(p) && mark(p).abs() > mm(20) && p.sub(o).dot(fwd) < mm(140) {
            mm(34)
        } else {
            0
        }
    };
    fray(s, &snout, &nap, mm(26), &muzzle_tuft, mark);
    coat(s, snout, |_| ONE);

    s.brush(NOSE, OWN, HEAD);
    s.ball(
        h(168, -8, 0),
        v3(mm(23), mm(17), mm(17)),
        &Affine::rot_x(deg(20)),
        2,
    );
    s.brush(GLINT, OWN, HEAD);
    s.ball(h(176, 3, 6), v3(mm(5), mm(3), mm(4)), &Affine::IDENTITY, 1);
    for x in [-1, 1] {
        let c = h(70, 48, x * 58);
        let look = fwd
            .scale(ONE * 72 / 100)
            .add(side().scale(x * ONE * 66 / 100))
            .add(up.scale(ONE / 8))
            .norm();
        // sunk in the skull, with a dark lid round it and a soft brow over it
        s.brush(NOSE, OWN, HEAD);
        s.ball(
            c.sub(look.scale(mm(7))),
            v3(mm(20), mm(19), mm(20)),
            &Affine::IDENTITY,
            2,
        );
        s.brush(IRIS, OWN, HEAD);
        s.ball(c, v3(mm(15), mm(15), mm(15)), &Affine::IDENTITY, 2);
        s.brush(PUPIL, OWN, HEAD);
        s.ball(
            c.add(look.scale(mm(7))),
            v3(mm(10), mm(10), mm(10)),
            &Affine::IDENTITY,
            2,
        );
        s.brush(GLINT, OWN, HEAD);
        s.ball(
            c.add(look.scale(mm(14)))
                .add(up.scale(mm(5)))
                .add(fwd.scale(mm(2))),
            v3(mm(4), mm(4), mm(4)),
            &Affine::IDENTITY,
            1,
        );
        let mut brow = piece(HEAD);
        brow.ball(
            c.add(up.scale(mm(19))).sub(look.scale(mm(3))),
            v3(mm(22), mm(9), mm(16)),
            &Affine::rot_z(deg(x * 12)),
            2,
        );
        coat(s, brow, mark);
    }

    // a mouth a little open, a soft white jaw, the tongue lolling over its lip
    s.brush(MOUTH, OWN, HEAD);
    s.ball(h(108, -48, 0), v3(mm(38), mm(15), mm(62)), &Affine::IDENTITY, 2);
    let mut jaw = piece(JAW);
    jaw.tube(
        &fine(
            &[
                oval(h(46, -38, 0), 40, 18),
                oval(h(112, -58, 0), 34, 15),
                oval(h(150, -64, 0), 25, 11),
                oval(h(162, -65, 0), 10, 6),
            ],
            FINER,
        ),
        LIMB,
        side(),
    );
    coat(s, jaw, |_| ONE);
    s.brush(TONGUE, OWN, JAW);
    // one tongue, from inside the mouth over the lip and down
    s.ball(
        h(142, -48, 6),
        v3(mm(19), mm(8), mm(34)),
        &Affine::rot_x(deg(6)),
        2,
    );
    s.ball(
        h(170, -58, 8),
        v3(mm(18), mm(12), mm(15)),
        &Affine::rot_x(deg(30)),
        2,
    );
    s.ball(
        h(176, -76, 8),
        v3(mm(17), mm(20), mm(9)),
        &Affine::rot_x(deg(10)),
        2,
    );

    // soft ears: upright from a broad base, the top third tipped forward
    for (x, bone) in [(1, EAR_LEFT), (-1, EAR_RIGHT)] {
        let e = |out: i32, rise: i32, ahead: i32| h(ahead - 20, 80 + rise, x * (58 + out));
        let mut ear = piece(bone);
        ear.tube(
            &fine(
                &[
                    oval(e(-4, -18, 0), 36, 22),
                    oval(e(4, 12, -2), 35, 18),
                    oval(e(9, 36, 2), 28, 15),
                    oval(e(11, 50, 16), 22, 13),
                    oval(e(12, 46, 36), 16, 10),
                    oval(e(12, 34, 48), 5, 4),
                ],
                FINER,
            ),
            SOFT,
            side(),
        );
        coat(s, ear, |_| -ONE);
        // the warm inside of the ear, seen where it faces forward
        s.brush(EAR_IN, OWN, bone);
        s.ball(e(2, 6, 12), v3(mm(22), mm(26), mm(8)), &Affine::IDENTITY, 2);
    }
}

/// The Sheepdog.
pub fn sheepdog() -> Shape {
    let sk = rig::skeleton(Rig::Dog);
    let mut s = Shape::new();
    trunk(&mut s);
    ruff(&mut s);
    legs(&mut s, &sk);
    tail(&mut s, &sk);
    head(&mut s, &sk);
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::Vertex;
    use crate::render::geom::cm;

    /// The white side of the coat, whatever the light does to it.
    fn pale(c: Rgb) -> bool {
        c[0] > ONE / 2
    }

    fn cut(faces: &[[V3; 3]], mark: impl Fn(V3) -> i32) -> Vec<Vertex> {
        let mut part = piece(BODY);
        for f in faces {
            part.tri(f[0], f[1], f[2]);
        }
        let mut s = Shape::new();
        coat(&mut s, part, mark);
        s.finish().0
    }

    #[test]
    fn a_coat_mark_cuts_a_face_on_its_line() {
        // a triangle from y -1 to y 1, white above y 0: its two long edges
        // cross the line halfway, at x 0.5 and x -0.5
        let face = [v3(0, -ONE, 0), v3(ONE, ONE, 0), v3(-ONE, ONE, 0)];
        let vs = cut(&[face], |p| p.y);
        assert!(vs.iter().all(|v| pale(v.color) == (v.pos.y >= 0) || v.pos.y == 0));
        for x in [ONE / 2, -ONE / 2] {
            for white in [true, false] {
                let on_line = |v: &Vertex| v.pos == v3(x, 0, 0) && pale(v.color) == white;
                assert!(vs.iter().any(on_line), "{x} {white}");
            }
        }
    }

    #[test]
    fn two_faces_cut_their_shared_edge_at_one_point() {
        // the two faces reach the edge from opposite ends, and a third of the
        // way along is not a whole number of steps: no crack may open there
        let (a, b) = (v3(0, -ONE, 0), v3(ONE / 7, ONE * 2, 0));
        // one face has `a` alone below the line, the other `b` alone above
        let faces = [[a, b, v3(ONE, ONE, 0)], [b, a, v3(-ONE, -ONE / 2, 0)]];
        let vs = cut(&faces, |p| p.y);
        let mut on_line: Vec<V3> = vs
            .iter()
            .filter(|v| v.pos.y.abs() < 8 && v.pos.x.abs() < ONE / 7)
            .map(|v| v.pos)
            .collect();
        on_line.sort();
        on_line.dedup();
        assert_eq!(on_line.len(), 1, "{on_line:?}");
    }

    fn built() -> Vec<Vertex> {
        sheepdog().finish().0
    }

    /// The vertex highest up, or lowest down, among those that pass `keep`.
    fn top(vs: &[Vertex], keep: impl Fn(&Vertex) -> bool) -> Rgb {
        vs.iter()
            .filter(|v| keep(v))
            .max_by_key(|v| v.pos.y)
            .map_or(GLINT, |v| v.color)
    }

    fn bottom(vs: &[Vertex], keep: impl Fn(&Vertex) -> bool) -> Rgb {
        vs.iter()
            .filter(|v| keep(v))
            .min_by_key(|v| v.pos.y)
            .map_or(GLINT, |v| v.color)
    }

    #[test]
    fn from_above_a_sheepdog_is_a_black_back_a_white_collar_and_a_white_tail_tip() {
        let vs = built();
        assert!(!pale(top(&vs, |v| v.bone == BODY)), "the back");
        assert!(pale(top(&vs, |v| v.bone == NECK)), "the collar");
        let far = vs.iter().filter(|v| v.bone == TAIL[2]).min_by_key(|v| v.pos.z);
        assert!(far.is_some_and(|v| pale(v.color)), "the tail's tip");
        assert!(!pale(top(&vs, |v| v.bone == TAIL[0])), "the tail's root");
    }

    #[test]
    fn a_sheepdog_has_a_white_belly_and_four_white_paws() {
        let vs = built();
        assert!(pale(bottom(&vs, |v| v.bone == BODY)), "the belly");
        for leg in 0..4 {
            assert!(pale(bottom(&vs, |v| v.bone == leg_bone(leg, 2))), "paw {leg}");
        }
        assert!(!pale(top(&vs, |v| v.bone == leg_bone(2, 0))), "the thigh");
    }

    #[test]
    fn two_lumps_of_one_height_melt_above_it() {
        // at equal heights the meld adds a quarter of MELT: 1/12 of a meter
        // over 1/5; far apart in height it adds nothing
        let got = soft_max(ONE / 5, ONE / 5);
        assert!((got - (ONE / 5 + ONE / 12)).abs() < 8, "{got}");
        assert_eq!(soft_max(ONE, 0), ONE);
    }

    #[test]
    fn fur_lifts_a_corner_along_its_normal_and_no_further() {
        // a flat face looking up: a rise of 1 cm moves corners up by at most
        // that, and never sideways
        let (a, b, c) = (v3(0, 0, 0), v3(0, 0, -cm(40)), v3(cm(40), 0, 0));
        let mut part = piece(BODY);
        part.smooth_tri([a, b, c], [v3(0, ONE, 0); 3]);
        let nap = Nap {
            flow: &|_| v3(0, 0, -ONE),
            rise: &|_, _| cm(1),
            long: cm(30),
            wide: cm(30),
            cell: cm(10),
            edge: 0,
        };
        let mut s = Shape::new();
        lay(&mut s, part, &nap, |_| -ONE);
        let vs = s.finish().0;
        assert_eq!(vs.len(), 3);
        for v in &vs {
            assert!(v.pos.y >= 0 && v.pos.y <= cm(1), "{}", v.pos.y);
            assert!(v.pos.x % cm(40) == 0 && v.pos.z % cm(40) == 0);
        }
        assert!(vs.iter().any(|v| v.pos.y > 0), "and it does lift them");
    }

    #[test]
    fn a_tuft_grows_for_each_gap_squared_and_none_where_it_has_no_length() {
        // a face of 0.08 square meters at a gap of 0.1 m holds 8 tufts, less
        // the last if the sum rounds down, of 4 triangles each
        let (a, b, c) = (v3(0, 0, 0), v3(0, 0, -cm(40)), v3(cm(40), 0, 0));
        let mut part = piece(BODY);
        part.smooth_tri([a, b, c], [v3(0, ONE, 0); 3]);
        let nap = Nap {
            flow: &|_| v3(0, 0, -ONE),
            rise: &|_, _| 0,
            long: ONE,
            wide: ONE,
            cell: 0,
            edge: 0,
        };
        let mut s = Shape::new();
        fray(&mut s, &part, &nap, cm(10), &|_, _| cm(5), |_| -ONE);
        assert!([28, 32].contains(&s.triangles()), "{}", s.triangles());
        let mut none = Shape::new();
        fray(&mut none, &part, &nap, cm(10), &|_, _| 0, |_| -ONE);
        assert_eq!(none.triangles(), 0);
    }

    #[test]
    fn a_sheepdog_stays_in_its_triangle_budget() {
        // 8 are drawn at most, with their shadows and outlines
        let n = sheepdog().triangles();
        assert!(n < 32_000, "{n}");
    }

    #[test]
    fn a_white_blaze_runs_down_the_middle_of_a_black_head() {
        let vs = built();
        // the crown of the skull, over the eyes: the middle and the sides
        let forehead = |v: &Vertex| v.bone == HEAD && v.pos.z > cm(34) && v.pos.z < cm(38);
        assert!(pale(top(&vs, |v| forehead(v) && v.pos.x.abs() < mm(8))));
        assert!(!pale(top(&vs, |v| forehead(v) && v.pos.x.abs() > mm(40))));
    }
}
