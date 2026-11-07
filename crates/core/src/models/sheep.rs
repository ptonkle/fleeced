//! Cloud sheep: a soft round egg of wool on thick short legs, a round face,
//! drooping ears, a pompom tail. The wool is one closed skin with soft lumps
//! melted into it (`puff`), so 400 sheep cost a skin each and the outline pass
//! gets one clean silhouette. The colors stay the Shetland breed's.

use crate::fx::{self, mul, ONE, TURN};
use crate::mesh::{icosphere, rgb, ring, shade, Rgb, Shape, COAT, OWN, SKIN};
use crate::render::geom::{cm, deg, mm, smooth, v3, Affine, V3};

use super::rig::{self, leg_bone, Leg, Rig, Skeleton, BELL, EAR_LEFT, EAR_RIGHT, HEAD, NECK, TAIL};

const HOOF: Rgb = rgb(52, 44, 40);
/// The nose: a warm shade of the sheep's own skin, shares of it.
pub const NOSE: Rgb = [cm(76), cm(58), cm(58)];
pub const EYE: Rgb = rgb(30, 22, 20);
pub const GLINT: Rgb = rgb(250, 248, 240);
const HORN: Rgb = rgb(196, 182, 150);
const LEATHER: Rgb = rgb(92, 56, 34);
const BRASS: Rgb = rgb(214, 170, 78);
const BRASS_DARK: Rgb = rgb(150, 110, 48);

/// The fleece in cm: half width, half height, half length, how far it hangs.
const WOOL: (i32, i32, i32, i32) = (31, 23, 42, 4);
/// How many lumps, how big each is across (cm) and how far each stands (mm).
const LUMPS: i32 = 18;
const LUMP: i32 = 14;
pub const RISE: i32 = 34;
/// A crease's tone, percent of the wool.
const CREASE: i32 = 84;
/// How fine the fleece is at each level of detail.
pub const WRAP: [u32; 3] = [3, 3, 2];
/// How far the face tips down, degrees.
const TILT: i32 = 30;
/// The eye's radius, then along the face, up it and out from the middle, mm.
const EYE_R: i32 = 17;
const EYE_AT: (i32, i32, i32) = (52, 24, 50);
/// The ear's half length (mm) and how far it droops (degrees).
const EAR_LONG: i32 = 56;
const EAR_DROOP: i32 = 22;

/// A repeatable wobble from -ONE to ONE: a hash of `i`.
pub fn noise(i: i32) -> i32 {
    let h = (i as u32).wrapping_add(0x9E37).wrapping_mul(2_654_435_761);
    let h = (h ^ (h >> 15)).wrapping_mul(2_246_822_519);
    ((h >> 16) as i32 & 0xFFFF) * 2 - ONE
}

/// One sphere of a cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lump {
    pub at: V3,
    pub r: i32,
}

/// `count` lumps of about radius `r` spread evenly over the ellipsoid of
/// `inner` radii round `center`: the sunflower spiral, each a little off.
pub fn lumps(center: V3, inner: V3, r: i32, count: i32, seed: i32) -> Vec<Lump> {
    (0..count)
        .map(|i| {
            let y = ONE - fx::ratio(2 * i + 1, count);
            let round = fx::sqrt(ONE - mul(y, y));
            let a = i * 25_033 + seed;
            let unit = v3(mul(round, fx::cos(a)), y, mul(round, fx::sin(a)));
            let far = ONE + noise(i * 3 + seed) / 8;
            Lump {
                at: center.add(unit.times(inner).scale(far)),
                r: mul(r, ONE + noise(i * 7 + seed) / 5),
            }
        })
        .collect()
}

/// The hard-creased skin of a cluster of lumps round `center`, for the
/// Carcass's torn tuft. `radii` is the ball the rays start from: its shape
/// spreads the corners over the body.
pub fn fleece(s: &mut Shape, center: V3, radii: V3, cluster: &[Lump], detail: u32) {
    let (points, faces) = icosphere(detail);
    let corners: Vec<(V3, V3, i32)> = points
        .iter()
        .map(|u| {
            let dir = u.times(radii).norm();
            // an under-layer of wool where a ray misses every lump
            let under = mul(u.times(radii).len(), cm(84));
            let (mut best, mut normal, mut top) = (under, dir, cm(55));
            for l in cluster {
                let m = l.at.sub(center);
                let along = m.dot(dir);
                let off = m.dot(m) - mul(along, along);
                let gap = mul(l.r, l.r) - off;
                if gap > 0 {
                    let t = along + fx::sqrt(gap);
                    if t > best {
                        best = t;
                        normal = dir.scale(t).sub(m).norm();
                        top = normal.dot(dir);
                    }
                }
            }
            let p = center.add(dir.scale(best));
            // soft: the lump's own normal, leaning toward the ray
            (p, normal.add(dir.scale(cm(35))).norm(), top)
        })
        .collect();
    // coarse meshes have wide creases: gentler, or a far sheep goes blotchy
    let floor = 56 + (4 - detail as i32) * 14;
    let (tint, patch) = (s.color, s.patch);
    let tone = |at: V3, top: i32| {
        let light = cm(floor) + mul(cm(100 - floor), smooth(cm(46), cm(96), top));
        let c = patch.map_or(tint, |f| f.at(at));
        [mul(c[0], light), mul(c[1], light), mul(c[2], light)]
    };
    for f in faces {
        let (a, b, c) = (corners[f[0]], corners[f[1]], corners[f[2]]);
        s.painted_tri(
            [a.0, b.0, c.0],
            [a.1, b.1, c.1],
            [tone(a.0, a.2), tone(b.0, b.2), tone(c.0, c.2)],
        );
    }
}

/// The softer of two heights: rounds the join where lumps meet.
fn soft_max(a: i32, b: i32, k: i32) -> i32 {
    let h = (k - (a - b).abs()).max(0);
    if k == 0 {
        a.max(b)
    } else {
        a.max(b) + mul(mul(h, h), fx::div(ONE, k)) / 4
    }
}

/// One closed soft skin: an egg of `radii` round `center`, raised by `rise`
/// over each lump and melted between them, so no crease is ever a hard edge.
/// The sheep's wool, and the disguise's.
pub fn puff(s: &mut Shape, center: V3, radii: V3, cluster: &[Lump], rise: i32, detail: u32) {
    let (points, faces) = icosphere(detail);
    let bend = v3(
        fx::div(ONE, radii.x),
        fx::div(ONE, radii.y),
        fx::div(ONE, radii.z),
    );
    let corners: Vec<(V3, i32)> = points
        .iter()
        .map(|u| {
            let p = u.times(radii);
            let out = u.times(bend).norm();
            let mut height = 0;
            for l in cluster {
                let near = ONE - fx::div(p.sub(l.at).len(), l.r).min(ONE);
                let h = mul(rise, smooth(0, cm(70), near));
                height = soft_max(height, h, rise / 2);
            }
            (p.add(out.scale(height)), height)
        })
        .collect();
    let mut normals = vec![V3::ZERO; corners.len()];
    for f in &faces {
        let (a, b, c) = (corners[f[0]].0, corners[f[1]].0, corners[f[2]].0);
        let wide = fx::from_int(8);
        let n = b.sub(a).scale(wide).cross(c.sub(a).scale(wide));
        for k in f {
            normals[*k] = normals[*k].add(n);
        }
    }
    let (tint, floor) = (s.color, cm(CREASE));
    let tone = |height: i32| {
        let lit = floor + mul(ONE - floor, smooth(0, rise * 3 / 4, height));
        [mul(tint[0], lit), mul(tint[1], lit), mul(tint[2], lit)]
    };
    let corner = |k: usize| {
        let (p, h) = corners[k];
        let out = points[k].times(bend).norm();
        (
            center.add(p),
            normals[k].norm().add(out.scale(cm(35))).norm(),
            tone(h),
        )
    };
    for f in faces {
        let (a, b, c) = (corner(f[0]), corner(f[1]), corner(f[2]));
        s.painted_tri([a.0, b.0, c.0], [a.1, b.1, c.1], [a.2, b.2, c.2]);
    }
}

fn at3(p: (i32, i32), x: i32) -> V3 {
    v3(x, p.1, p.0)
}

/// A thick rounded leg in the coat's skin, a dark hoof. One color down to the
/// hoof: a wool thigh read as a bandage wherever the knee showed.
fn leg(s: &mut Shape, l: &Leg, index: usize, k: i32, sides: i32, near: bool) {
    let x = if index.is_multiple_of(2) { l.x } else { -l.x };
    let side = v3(ONE, 0, 0);
    let r = |n: i32| mul(mm(n), k);
    let (thigh, shank, hoof) = (r(64), r(56), r(58));
    s.brush([ONE; 3], SKIN, leg_bone(index, 0));
    // narrow at the hip, inside the fleece, then the full thigh
    let (hip, knee) = (at3(l.hip, x), at3(l.knee, x));
    let round = (thigh + shank) / 2;
    s.tube(
        &[
            ring(hip, thigh / 2),
            ring(hip.lerp(knee, cm(45)), thigh),
            ring(knee, round),
        ],
        sides,
        side,
    );
    s.brush([ONE; 3], SKIN, leg_bone(index, 1));
    s.tube(&[ring(knee, round), ring(at3(l.ankle, x), shank)], sides, side);
    let joint = round * 19 / 20;
    s.ball(knee, v3(joint, joint, joint), &Affine::IDENTITY, near as u32);
    s.brush([ONE; 3], SKIN, leg_bone(index, 2));
    let pastern = (l.foot.0, l.foot.1 + hoof * 4 / 5);
    s.tube(
        &[ring(at3(l.ankle, x), shank), ring(at3(pastern, x), hoof)],
        sides,
        side,
    );
    s.brush(HOOF, OWN, leg_bone(index, 2));
    let rim = (l.foot.0 + hoof / 5, l.foot.1 + hoof / 3);
    let sole = (l.foot.0 + hoof / 5, l.foot.1);
    s.tube(
        &[
            ring(at3(pastern, x), hoof),
            ring(at3(rim, x), hoof + hoof / 8),
            ring(at3(sole, x), hoof),
        ],
        sides,
        side,
    );
}

/// Where the face points, and the head's own up.
fn head_frame(tilt: i32) -> (V3, V3) {
    let down = deg(tilt);
    (
        v3(0, -fx::sin(down), fx::cos(down)),
        v3(0, fx::cos(down), fx::sin(down)),
    )
}

/// A round head on the neck's end: one ball for the skull, one for the muzzle,
/// a soft nose in the sheep's own skin, two big gentle eyes with a heavy lid,
/// soft ears. `k` scales it.
fn head(s: &mut Shape, sk: &Skeleton, k: i32, near: bool) {
    let (fwd, up) = head_frame(TILT);
    let side = v3(ONE, 0, 0);
    let o = sk.head;
    let r = |n: i32| mul(mm(n), k);
    let h = |along: i32, high: i32, x: i32| {
        o.add(fwd.scale(r(along)))
            .add(up.scale(r(high)))
            .add(side.scale(r(x)))
    };
    let turn = Affine::rot_x(deg(TILT));
    let (detail, small) = if near { (2, 1) } else { (1, 0) };
    s.brush([ONE; 3], SKIN, HEAD);
    s.ball(h(24, 12, 0), v3(r(62), r(60), r(66)), &turn, detail);
    s.ball(h(90, -8, 0), v3(r(46), r(40), r(64)), &turn, detail);
    // the nose: no dark button, the face's own skin a warmer shade
    s.brush(NOSE, SKIN, HEAD);
    s.ball(h(146, 10, 0), v3(r(21), r(13), r(11)), &turn, small);
    let (along, high, wide) = EYE_AT;
    for x in [-wide, wide] {
        let out = x.signum();
        s.brush(EYE, OWN, HEAD);
        s.ball(h(along, high, x), v3(r(EYE_R), r(EYE_R), r(EYE_R)), &turn, small);
        if near {
            // the lid lies over the top of the eye, which is what makes it mild
            s.brush([ONE; 3], SKIN, HEAD);
            let lid = v3(r(EYE_R + 1), r(EYE_R / 2), r(EYE_R + 1));
            s.ball(h(along, high + EYE_R * 3 / 4, x), lid, &turn, 1);
            s.brush(GLINT, OWN, HEAD);
            let spot = EYE_R / 3;
            s.ball(
                h(along + EYE_R - 2, high, x - out * spot / 2),
                v3(r(spot), r(spot), r(spot / 2)),
                &turn,
                0,
            );
        }
    }
    for (x, bone) in [(1, EAR_LEFT), (-1, EAR_RIGHT)] {
        s.brush([ONE; 3], SKIN, bone);
        let drop = fx::sin(deg(EAR_DROOP));
        let at = h(-4, 36 - mul(EAR_LONG, drop), x * (58 + EAR_LONG));
        let ear = Affine::rot_z(deg(x * -EAR_DROOP)).then(&Affine::rot_y(deg(x * 14)));
        s.ball(at, v3(r(EAR_LONG), r(12), r(27)), &ear, small);
    }
    s.brush([ONE; 3], COAT, HEAD);
    s.ball(h(-14, 66, 0), v3(r(44), r(30), r(44)), &turn, detail);
}

/// A short thick curl on each side of a ram's head.
fn horns(s: &mut Shape, sk: &Skeleton, near: bool) {
    let (fwd, up) = head_frame(TILT);
    let steps = if near { 9 } else { 6 };
    for x in [ONE, -ONE] {
        let spiral: Vec<crate::mesh::Ring> = (0..=steps)
            .map(|step| {
                let t = fx::ratio(step, steps);
                let a = mul(t, TURN) + TURN / 12;
                let r = cm(9) - mul(cm(4), t);
                let middle = sk.head.add(fwd.scale(-cm(1))).add(up.scale(cm(1)));
                let out = cm(6) + mul(cm(8), t);
                let at = middle
                    .add(up.scale(mul(fx::cos(a), r)))
                    .add(fwd.scale(-mul(fx::sin(a), r)))
                    .add(v3(mul(x, out), 0, 0));
                ring(at, cm(4) - mul(cm(3), t))
            })
            .collect();
        s.brush(HORN, OWN, HEAD);
        s.tube(&spiral, if near { 8 } else { 6 }, v3(ONE, 0, 0));
        // a round root, not a cut end
        s.ball(spiral[0].at, v3(cm(4), cm(4), cm(4)), &Affine::IDENTITY, 1);
    }
}

/// A round pompom of a tail, on any rig's two tail joints.
pub fn tail(s: &mut Shape, sk: &Skeleton, k: i32, near: bool) {
    let [a, b, _] = sk.tail;
    s.brush([ONE; 3], COAT, TAIL[0]);
    s.ball(
        a,
        v3(mul(cm(6), k), mul(cm(6), k), mul(cm(6), k)),
        &Affine::IDENTITY,
        near as u32,
    );
    s.brush(shade([ONE; 3], 96), COAT, TAIL[1]);
    let r = mul(cm(7), k);
    s.ball(b, v3(r, r, r), &Affine::IDENTITY, if near { 2 } else { 1 });
}

/// The wool and the legs: `size` scales the fleece and neck, `leg_k` the legs.
fn body(s: &mut Shape, sk: &Skeleton, size: i32, leg_k: i32, level: u32) {
    let near = level < 2;
    let sides = [8, 7, 6][level as usize];
    let wrap = WRAP[level as usize];
    let radii = v3(cm(WOOL.0), cm(WOOL.1), cm(WOOL.2)).scale(size);
    let center = sk.body.add(v3(0, -mul(cm(WOOL.3), size), 0));
    let cluster = lumps(V3::ZERO, radii, mul(cm(LUMP), size), LUMPS, 7);
    s.brush([ONE; 3], COAT, rig::BODY);
    puff(s, center, radii, &cluster, mul(mm(RISE), size), wrap);
    let middle = sk.neck.lerp(sk.head, cm(46));
    neck_wool(s, middle, size, wrap - 1);
    for index in 0..4 {
        let l = if index < 2 { &sk.fore } else { &sk.hind };
        leg(s, l, index, leg_k, sides, near);
    }
    tail(s, sk, size, near);
}

/// The soft ruff round the neck, on the neck's own bone.
pub fn neck_wool(s: &mut Shape, middle: V3, size: i32, detail: u32) {
    s.brush([ONE; 3], COAT, NECK);
    let radii = v3(cm(15), cm(16), cm(15)).scale(size);
    let collar = lumps(V3::ZERO, radii, mul(cm(LUMP), size) / 2, LUMPS / 2, 91);
    puff(s, middle, radii, &collar, mul(mm(RISE), size) / 2, detail);
}

/// An adult sheep; `horned` for a ram with the curled horns.
pub fn adult(horned: bool, level: u32) -> Shape {
    let sk = rig::skeleton(Rig::Sheep);
    let mut s = Shape::new();
    let near = level < 2;
    body(&mut s, &sk, ONE, ONE, level);
    head(&mut s, &sk, ONE, near);
    if horned {
        horns(&mut s, &sk, near);
    }
    s
}

/// A lamb: a small soft fleece, short legs and a big head.
pub fn lamb(level: u32) -> Shape {
    let sk = rig::skeleton(Rig::Lamb);
    let mut s = Shape::new();
    body(&mut s, &sk, cm(58), cm(72), level);
    head(&mut s, &sk, cm(78), level < 2);
    s
}

/// A leather collar with a small brass bell, on the neck over the wool: the
/// Bell powerup. Worn as a second mesh with the sheep's own pose, so it
/// stays on whatever the sheep does.
pub fn bell(rig_kind: Rig, level: u32) -> Shape {
    let near = level < 2;
    let sk = rig::skeleton(rig_kind);
    let k = if rig_kind == Rig::Lamb { cm(60) } else { ONE };
    let mut s = Shape::new();
    let middle = sk.neck.lerp(sk.head, cm(38));
    let axis = sk.head.sub(sk.neck).norm();
    let side = v3(ONE, 0, 0);
    let under = side.cross(axis).norm();
    let reach = mul(cm(17), k);
    let around = if near { 16 } else { 10 };
    let mut band: Vec<crate::mesh::Ring> = (0..=around)
        .map(|i| {
            let a = TURN * i / around;
            let wide = mul(reach, cm(96));
            let at = middle
                .add(side.scale(mul(fx::cos(a), wide)))
                .add(under.scale(mul(fx::sin(a), reach)));
            ring(at, mul(cm(2), k))
        })
        .collect();
    band[around as usize] = band[0];
    s.brush(LEATHER, OWN, NECK);
    s.tube(&band, 5, axis);
    // hangs from the front of the collar, swinging on its own bone
    let hang = middle.add(under.scale(reach));
    let down = v3(0, -ONE, 0);
    let at = |d: i32| hang.add(down.scale(mul(mm(d), k)));
    s.brush(BRASS_DARK, OWN, BELL);
    s.ball(
        at(4),
        v3(mul(mm(12), k), mul(mm(10), k), mul(mm(12), k)),
        &Affine::IDENTITY,
        1,
    );
    s.brush(BRASS, OWN, BELL);
    let sides = if near { 10 } else { 6 };
    let r = |n: i32| mul(mm(n), k);
    s.tube(
        &[
            ring(at(10), r(14)),
            ring(at(20), r(28)),
            ring(at(38), r(33)),
            ring(at(56), r(38)),
            ring(at(66), r(48)),
        ],
        sides,
        side,
    );
    s.brush(BRASS_DARK, OWN, BELL);
    s.ball(at(70), v3(r(16), r(16), r(16)), &Affine::IDENTITY, 1);
    s
}

/// A Shetland's color: its wool and its bare skin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Coat {
    pub wool: Rgb,
    pub skin: Rgb,
}

/// The breed's real range, lightest first. Cream white is what lets a Wolf
/// hide: it is most of the flock.
pub const COATS: [Coat; 8] = [
    // white, a pale face
    Coat {
        wool: rgb(238, 232, 218),
        skin: rgb(226, 206, 186),
    },
    // white with a darker face
    Coat {
        wool: rgb(232, 224, 206),
        skin: rgb(124, 100, 82),
    },
    // fawn
    Coat {
        wool: rgb(196, 170, 136),
        skin: rgb(150, 118, 92),
    },
    // light gray
    Coat {
        wool: rgb(176, 174, 168),
        skin: rgb(126, 118, 110),
    },
    // gray
    Coat {
        wool: rgb(136, 134, 130),
        skin: rgb(84, 78, 74),
    },
    // dark gray
    Coat {
        wool: rgb(96, 94, 94),
        skin: rgb(62, 58, 56),
    },
    // moorit: the breed's red brown
    Coat {
        wool: rgb(112, 76, 52),
        skin: rgb(70, 48, 36),
    },
    // black
    Coat {
        wool: rgb(48, 44, 46),
        skin: rgb(38, 34, 36),
    },
];

/// Share of a flock in each of `COATS`, out of 100: a same-y flock, about 85
/// cream whites (the first two) and about 15 odd sheep.
const COAT_SHARE: [i32; 8] = [68, 17, 3, 3, 3, 0, 3, 3];
/// The cream whites come first in `COATS`.
pub const CREAM_COATS: usize = 2;
/// How far one cream sheep strays from the next, percent either way: enough
/// that a flock is not a clone army.
const CREAM_TONE: i32 = 4;

/// Which of `COATS` a sheep wears, from a number that names it.
pub fn coat_index(seed: i32) -> usize {
    let mut roll = (noise(seed) + ONE) * 50 / ONE;
    for (k, share) in COAT_SHARE.iter().enumerate() {
        roll -= share;
        if roll < 0 {
            return k;
        }
    }
    0
}

/// A sheep's coat from a number that names it: a cream white has its own
/// tone, lighter or darker and a touch warmer or cooler.
pub fn coat_of(seed: i32) -> Coat {
    let k = coat_index(seed);
    let c = COATS[k];
    if k >= CREAM_COATS {
        return c;
    }
    let light = ONE + noise(seed * 13 + 5) * CREAM_TONE / 100;
    let warm = noise(seed * 17 + 9) * CREAM_TONE / 200;
    let [r, g, b] = c.wool;
    Coat {
        wool: [
            mul(mul(r, light), ONE + warm),
            mul(g, light),
            mul(mul(b, light), ONE - warm),
        ],
        skin: c.skin,
    }
}

/// How much bigger than the model a sheep is, from a number that names it:
/// within 12% either way.
pub fn size_of(seed: i32) -> i32 {
    ONE + mul(noise(seed * 5 + 3), cm(12))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_soft_join_is_above_both_heights_only_near_where_they_meet() {
        let (a, b) = (cm(10), cm(9));
        assert!(soft_max(a, b, cm(4)) > a);
        assert_eq!(soft_max(cm(30), b, cm(4)), cm(30));
        assert_eq!(soft_max(a, b, 0), a);
    }
}
