//! The Wolf, painted with the instance's coat so it wears the flock's cream: a
//! darker saddle, pale flanks and legs.
//!
//! The plain Wolf is one low-poly pelt from nose to tail tip. Every part is a
//! faceted skin under rows of one tuft, laid along the hair like shingles
//! ([`pelt`]). The tuft is the same size on the face, the legs and the body,
//! so the triangles are too; a part shorter than a tuft is skin alone. Shape
//! comes from the skin's rings, never from finer fur.
//!
//! The Sheep Suit is the same animal in the sheep's own soft wool, thick legs,
//! dark feet and pompom tail, so only the snout and the pointed ears give it
//! away.

use crate::fx::{self, div, mul, ONE, TURN};
use crate::mesh::{face_normal, rgb, ring, Rgb, Ring, Shape, COAT, OWN};
use crate::render::geom::{cm, deg, lerp, mm, smooth, v3, Affine, V3};

use super::hiding::Hiding;
use super::rig::{
    self, leg_bone, Leg, Rig, Skeleton, BODY, CHEST, EAR_LEFT, EAR_RIGHT, HEAD, JAW, NECK, TAIL,
};
use super::sheep::{self, lumps, noise};

pub const NOSE: Rgb = rgb(24, 22, 24);
const PUPIL: Rgb = rgb(16, 14, 14);
pub const WOLF_EYE: Rgb = rgb(236, 168, 44);
pub const TOOTH: Rgb = rgb(240, 234, 216);
/// The Wolf's dark markings, shares of its coat: the mask, brow and ear backs,
/// then the hackles.
pub const MASK: Rgb = [cm(50); 3];
const HACKLE: Rgb = [cm(62); 3];
/// The one tuft at the nearest mesh, mm: long along the hair, half as wide
/// across. How far its tip stands off the skin is `Hiding::fur`.
const TUFT: (i32, i32) = (130, 46);
/// A tuft's shades in percent of the fur there: its root, the skin showing
/// between tufts, skin with no tufts on it, the shoulders of the tuft and its
/// tip.
const ROOT: i32 = 88;
const BARE: i32 = 84;
const PLAIN: i32 = 94;
const SHOULDER: i32 = 96;
const TIP: i32 = 102;
/// The disguise's legs and feet, shares of the coat: the cream sheep's bare
/// skin and dark hooves.
const SUIT_LEG: Rgb = [cm(95), cm(89), cm(85)];
const SUIT_FOOT: Rgb = [cm(24), cm(21), cm(20)];
/// The disguise's wool: its height and place along the body, and its half
/// height and half length, cm. Long enough for a Wolf, as round as a ewe.
const SUIT_AT: (i32, i32) = (58, -10);
const SUIT_SIZE: (i32, i32) = (22, 47);

fn at3(p: (i32, i32), x: i32) -> V3 {
    v3(x, p.1, p.0)
}

/// The disguise's leg: three tapered sections and a paw.
struct LegLook {
    upper: (i32, i32),
    lower: (i32, i32),
    foot: i32,
    paw: V3,
    sides: i32,
}

fn leg(s: &mut Shape, l: &Leg, index: usize, look: &LegLook) {
    let x = if index.is_multiple_of(2) { l.x } else { -l.x };
    let side = v3(ONE, 0, 0);
    s.bone = leg_bone(index, 0);
    s.tube(
        &[
            ring(at3(l.hip, x), look.upper.0),
            ring(at3(l.knee, x), look.upper.1),
        ],
        look.sides,
        side,
    );
    s.bone = leg_bone(index, 1);
    s.tube(
        &[
            ring(at3(l.knee, x), look.upper.1),
            ring(at3(l.ankle, x), look.lower.1),
        ],
        look.sides,
        side,
    );
    s.bone = leg_bone(index, 2);
    let toe = (l.foot.0, l.foot.1 + look.paw.y / 2);
    s.tube(
        &[ring(at3(l.ankle, x), look.lower.1), ring(at3(toe, x), look.foot)],
        look.sides,
        side,
    );
    s.color = SUIT_FOOT;
    let front = (l.foot.0 + look.paw.z / 3, l.foot.1 + look.paw.y);
    s.ball(at3(front, x), look.paw, &Affine::IDENTITY, 0);
}

/// A tuft: long along the hair, half as wide, and how far its tip stands off
/// the skin.
#[derive(Clone, Copy)]
struct Tuft {
    long: i32,
    half: i32,
    lift: i32,
}

/// How the fur lies on one part of the pelt.
#[derive(Clone, Copy)]
struct Nap<'a> {
    /// the hair runs toward the first ring, as it does back along the body;
    /// otherwise toward the last, as it does down a leg
    backward: bool,
    /// what the hair leans toward, besides along the part
    drift: V3,
    /// the one tuft
    tuft: Tuft,
    /// whether a tuft may lie on this point: nothing covers the eyes
    open: &'a dyn Fn(V3) -> bool,
    /// the fur's share of the coat there
    share: &'a dyn Fn(V3) -> i32,
    /// faces round the skin
    sides: i32,
    /// whether the skin's ends are closed; most sit inside the next part
    capped: bool,
    /// how far a mark's edge wanders, tuft to tuft
    blur: i32,
    seed: i32,
}

/// One ring of a skin with its frame, as `Shape::tube` carries it.
#[derive(Clone, Copy)]
struct Hoop {
    at: V3,
    side: V3,
    up: V3,
    along: V3,
    wide: i32,
    tall: i32,
    /// how fast the skin widens along the part
    flare: i32,
}

impl Hoop {
    /// The skin at an angle round the hoop: the point and its normal.
    fn place(&self, angle: i32) -> (V3, V3) {
        let (c, n) = (fx::cos(angle), fx::sin(angle));
        let at = self
            .at
            .add(self.side.scale(mul(c, self.wide)))
            .add(self.up.scale(mul(n, self.tall)));
        let normal = self
            .side
            .scale(mul(c, div(ONE, self.wide)))
            .add(self.up.scale(mul(n, div(ONE, self.tall))))
            .norm()
            .sub(self.along.scale(self.flare))
            .norm();
        (at, normal)
    }

    fn toward(&self, o: &Hoop, t: i32) -> Hoop {
        Hoop {
            at: self.at.lerp(o.at, t),
            side: self.side.lerp(o.side, t).norm(),
            up: self.up.lerp(o.up, t).norm(),
            along: self.along.lerp(o.along, t).norm(),
            wide: lerp(self.wide, o.wide, t),
            tall: lerp(self.tall, o.tall, t),
            flare: lerp(self.flare, o.flare, t),
        }
    }
}

/// `rings`, each framed along the part.
fn hoops(rings: &[Ring]) -> Vec<Hoop> {
    let last = rings.len() - 1;
    let mut side = v3(ONE, 0, 0);
    (0..=last)
        .map(|k| {
            let (before, after) = (rings[k.saturating_sub(1)], rings[(k + 1).min(last)]);
            let along = after.at.sub(before.at).norm();
            side = side.sub(along.scale(side.dot(along))).norm();
            let widens = (after.wide + after.tall - before.wide - before.tall) / 2;
            Hoop {
                at: rings[k].at,
                side,
                up: along.cross(side).norm(),
                along,
                wide: rings[k].wide,
                tall: rings[k].tall,
                flare: div(widens, after.at.dist(before.at).max(1)).clamp(-2 * ONE, 2 * ONE),
            }
        })
        .collect()
}

/// A shade of a share of the coat, in percent.
fn toned(share: i32, pct: i32) -> Rgb {
    [mul(share, cm(pct)); 3]
}

/// One flat facet with a color at each corner.
fn facet(s: &mut Shape, p: [V3; 3], c: [Rgb; 3]) {
    let n = face_normal(p[0], p[1], p[2]);
    s.painted_tri(p, [n; 3], c);
}

/// The skin: flat facets between the rings, darker where fur covers it.
fn skin(s: &mut Shape, hoops: &[Hoop], nap: &Nap, tone: i32) {
    let sides = nap.sides;
    let corner = |h: &Hoop, j: i32| {
        let (p, _) = h.place((j % sides) * TURN / sides);
        (p, toned((nap.share)(p), tone))
    };
    for pair in hoops.windows(2) {
        for j in 0..sides {
            let (a, b) = (corner(&pair[0], j), corner(&pair[0], j + 1));
            let (c, d) = (corner(&pair[1], j + 1), corner(&pair[1], j));
            facet(s, [a.0, b.0, c.0], [a.1, b.1, c.1]);
            facet(s, [a.0, c.0, d.0], [a.1, c.1, d.1]);
        }
    }
    if nap.capped {
        let (first, end) = (hoops[0], hoops[hoops.len() - 1]);
        for j in 0..sides {
            let (a, b) = (corner(&first, j), corner(&first, j + 1));
            s.painted_tri([first.at, b.0, a.0], [first.along.neg(); 3], [a.1; 3]);
            let (a, b) = (corner(&end, j), corner(&end, j + 1));
            s.painted_tri([end.at, a.0, b.0], [end.along; 3], [a.1; 3]);
        }
    }
}

/// One tuft: two faces meeting on a spine from a root in the skin at `p` to a
/// tip stood off it, lying along `hair`. The light stays close to the skin's,
/// so a tuft never flashes on its own.
fn tuft(s: &mut Shape, p: V3, n: V3, hair: V3, t: Tuft, share: i32) {
    let across = n.cross(hair).norm();
    let at = |along: i32, out: i32, off: i32| {
        p.add(hair.scale(mul(t.long, cm(along))))
            .add(across.scale(mul(t.half, cm(out))))
            .add(n.scale(mul(t.lift, cm(off))))
    };
    let (root, left, right, tip) = (at(0, 0, -10), at(36, 100, 35), at(36, -100, 35), at(100, 0, 100));
    let (dark, mid, lit) = (toned(share, ROOT), toned(share, SHOULDER), toned(share, TIP));
    let (up_left, up_right) = (
        n.add(across.scale(ONE / 3)).norm(),
        n.sub(across.scale(ONE / 3)).norm(),
    );
    s.painted_tri([root, tip, left], [up_left; 3], [dark, lit, mid]);
    s.painted_tri([root, right, tip], [up_right; 3], [dark, mid, lit]);
}

/// One part of the pelt: a skin through `rings` with rows of the one tuft
/// laid on it like shingles. The body, the legs, the head and the tail are
/// all this. A tuft runs at most a quarter of its length off an end, and
/// grows only where the part is as wide as it is, so a small part is skin
/// alone.
fn pelt(s: &mut Shape, rings: &[Ring], nap: &Nap) {
    let hoops = hoops(rings);
    let mut arc = vec![0];
    for pair in hoops.windows(2) {
        arc.push(arc[arc.len() - 1] + pair[0].at.dist(pair[1].at));
    }
    let length = arc[arc.len() - 1];
    let fits = length >= nap.tuft.long;
    skin(s, &hoops, nap, if fits { BARE } else { PLAIN });
    if !fits {
        return;
    }
    let flow = if nap.backward { -ONE } else { ONE };
    let reach = nap.tuft.long / 2;
    let (mut go, end) = if nap.backward {
        (reach, length)
    } else {
        (0, length - reach)
    };
    let (mut row, mut k) = (0, 0);
    while go <= end {
        while k + 2 < hoops.len() && arc[k + 1] < go {
            k += 1;
        }
        let f = div(go - arc[k], (arc[k + 1] - arc[k]).max(1)).min(ONE);
        let h = hoops[k].toward(&hoops[k + 1], f);
        // about 3(wide + tall) round the ring, a tuft and a little overlap
        let half = nap.tuft.half;
        let count = 3 * (h.wide + h.tall) / (half * 17 / 10);
        let count = if h.wide.max(h.tall) < half { 0 } else { count };
        for j in 0..count {
            let id = row * 131 + j * 17 + nap.seed;
            let slip = mul(noise(id), TURN / count / 3);
            let angle = (2 * j + (row & 1)) * (TURN / (2 * count)) + slip;
            let (p, n) = h.place(angle);
            let grown = |v: i32, salt: i32| mul(v, ONE * 7 / 8 + noise(id + salt) / 4);
            let t = Tuft {
                long: grown(nap.tuft.long, 1),
                half: grown(half, 2),
                lift: grown(nap.tuft.lift, 3),
            };
            let run = h.along.scale(flow).add(nap.drift);
            let lay = run.sub(n.scale(run.dot(n))).norm();
            let hair = lay.add(n.cross(lay).scale(noise(id + 4) / 5)).norm();
            let open = |along: i32| (nap.open)(p.add(hair.scale(along)));
            if !open(0) || !open(t.long / 2) || !open(t.long) {
                continue;
            }
            // a mark's edge breaks along tufts, not along a line
            let mark = p.add(v3(0, mul(noise(id + 5), nap.blur), mul(noise(id + 6), nap.blur)));
            let share = mul((nap.share)(mark), ONE * 24 / 25 + noise(id + 7) / 25);
            tuft(s, p, n, hair, t, share);
        }
        go += nap.tuft.long * 11 / 20;
        row += 1;
    }
}

/// What every part of the plain Wolf shares: the table and the one tuft.
struct Coat<'a> {
    t: &'a Hiding,
    level: u32,
    tuft: Tuft,
}

impl Coat<'_> {
    fn new(t: &Hiding, level: u32) -> Coat<'_> {
        // far off the grain coarsens, as a sheep's creases do
        let grain = t.grain * [100, 70, 40][level as usize] / 100;
        let coarse = 1800 / grain.max(1);
        Coat {
            t,
            level,
            tuft: Tuft {
                long: mm(TUFT.0 * coarse / 100),
                half: mm(TUFT.1 * coarse / 100),
                lift: mm(t.fur * 10),
            },
        }
    }

    /// Faces round a part, fewer far off.
    fn sides(&self, near: i32) -> i32 {
        (near * [100, 75, 50][self.level as usize] / 100).max(3)
    }

    fn nap<'a>(
        &self,
        backward: bool,
        drift: V3,
        share: &'a dyn Fn(V3) -> i32,
        sides: i32,
        seed: i32,
    ) -> Nap<'a> {
        Nap {
            backward,
            drift,
            tuft: self.tuft,
            open: &|_| true,
            share,
            sides: self.sides(sides),
            capped: false,
            blur: cm(4),
            seed,
        }
    }

    /// The fur's share of the coat by where it is: a dark saddle along the
    /// back and down over the rump, pale flanks and shoulders, darker legs,
    /// and the dark line of the hackles down the spine.
    fn share(&self, p: V3) -> i32 {
        let t = self.t;
        let back = smooth(cm(63), cm(73), p.y);
        let rump = mul(smooth(cm(38), cm(60), p.y), ONE - smooth(cm(-38), cm(-18), p.z));
        let saddle = mul(back.max(rump), ONE - smooth(cm(16), cm(30), p.z));
        let leg = ONE - smooth(cm(28), cm(44), p.y);
        let coat = lerp(lerp(cm(t.flank), cm(t.legs), leg), cm(t.back), saddle);
        let line = mul(ONE - smooth(cm(3), cm(7), p.x.abs()), smooth(cm(72), cm(76), p.y));
        lerp(coat, HACKLE[0], line)
    }
}

/// One side view of a leg: the place `pct` percent from `a` to `b`.
fn between(a: (i32, i32), b: (i32, i32), pct: i32) -> (i32, i32) {
    (lerp(a.0, b.0, cm(pct)), lerp(a.1, b.1, cm(pct)))
}

/// A real wolf's leg in one run of fur: a muscled top that melts into the
/// flank, a narrow cannon, the joints shaped into the line, and a paw with
/// toes.
fn wolf_leg(s: &mut Shape, coat: &Coat, l: &Leg, index: usize) {
    let hind = index >= 2;
    let x = if index.is_multiple_of(2) { l.x } else { -l.x };
    let thick = |n: i32| mm(n * coat.t.sturdy / 125);
    // a ring at a place in the side view, pushed back by `aft`, mm
    let hoop = |p: (i32, i32), out: i32, aft: i32, wide: i32, deep: i32| Ring {
        at: v3(mul(x, cm(out)), p.1, p.0 - mm(aft)),
        wide: thick(wide),
        tall: thick(deep),
    };
    let (hip, knee, ankle, foot) = (l.hip, l.knee, l.ankle, l.foot);
    let top = (hip.0, hip.1 + cm(5));
    let heel = (foot.0, foot.1 + cm(5));
    let (upper, lower, cannon) = if hind {
        // a deep thigh, the stifle forward, the gaskin narrowing to the hock's
        // point, then the long thin cannon
        (
            vec![
                hoop(top, 62, 0, 50, 90),
                hoop(between(hip, knee, 35), 92, 12, 60, 104),
                hoop(knee, 100, 0, 38, 54),
            ],
            vec![
                hoop(between(hip, knee, 95), 100, 0, 32, 46),
                hoop(between(knee, ankle, 40), 100, 3, 34, 40),
                hoop(ankle, 100, 8, 21, 27),
            ],
            vec![
                hoop(between(knee, ankle, 95), 100, 7, 18, 22),
                hoop(between(ankle, heel, 50), 100, 0, 18, 20),
                hoop(heel, 100, -6, 22, 24),
            ],
        )
    } else {
        // the shoulder blade under the skin, the elbow, the forearm's muscle
        // high up, the wrist, then the sloped pastern
        (
            vec![
                hoop(top, 70, -6, 44, 74),
                hoop(between(hip, knee, 40), 94, 0, 50, 72),
                hoop(knee, 100, 4, 36, 42),
            ],
            vec![
                hoop(between(hip, knee, 95), 100, 4, 30, 36),
                hoop(between(knee, ankle, 35), 100, 2, 30, 34),
                hoop(ankle, 100, 0, 23, 26),
            ],
            vec![
                hoop(between(knee, ankle, 95), 100, 0, 18, 20),
                hoop(between(ankle, heel, 40), 100, -2, 22, 23),
                hoop(heel, 100, -6, 22, 24),
            ],
        )
    };
    let share = |p: V3| coat.share(p);
    let aft = v3(0, 0, -ONE / 5);
    let nap = coat.nap(false, aft, &share, 5, 101 + index as i32 * 7);
    s.brush([ONE; 3], COAT, leg_bone(index, 0));
    pelt(s, &upper, &nap);
    s.bone = leg_bone(index, 1);
    let nap = Nap {
        sides: coat.sides(4),
        ..nap
    };
    pelt(
        s,
        &lower,
        &Nap {
            seed: nap.seed + 2,
            ..nap
        },
    );
    s.bone = leg_bone(index, 2);
    pelt(
        s,
        &cannon,
        &Nap {
            seed: nap.seed + 4,
            ..nap
        },
    );
    paw(s, coat, foot, x);
}

/// A paw: one faceted wedge from the heel to the toes, too small for a tuft.
fn paw(s: &mut Shape, coat: &Coat, foot: (i32, i32), x: i32) {
    let on = |ahead: i32, high: i32, wide: i32, tall: i32| Ring {
        at: v3(x, mm(high), foot.0 + mm(ahead)),
        wide: mm(wide),
        tall: mm(tall),
    };
    let share = |_: V3| cm(coat.t.legs) * 4 / 5;
    let nap = Nap {
        capped: true,
        ..coat.nap(false, V3::ZERO, &share, 5, 211)
    };
    pelt(
        s,
        &[on(-24, 44, 26, 18), on(14, 34, 38, 32), on(74, 18, 32, 16)],
        &nap,
    );
}

fn head_frame(down: i32) -> (V3, V3) {
    (
        v3(0, -fx::sin(down), fx::cos(down)),
        v3(0, fx::cos(down), fx::sin(down)),
    )
}

/// The Wolf. Fur takes the instance's coat, so a Wolf is the same cream as
/// the flock; `suit` dresses it as a sheep.
pub fn wolf(t: &Hiding, suit: bool, level: u32) -> Shape {
    let sk = rig::rest(Rig::Wolf);
    let mut s = Shape::new();

    s.flat = false;
    let coat = Coat::new(t, level);
    if suit {
        cloud_coat(&mut s, t, &sk, level);
        let thick = |n: i32| mm(n * t.disguise_legs / 100);
        let paw = v3(mm(52), mm(34), mm(56));
        let look_fore = LegLook {
            upper: (thick(46), thick(44)),
            lower: (thick(36), thick(29)),
            foot: thick(31),
            paw,
            sides: 6,
        };
        let look_hind = LegLook {
            upper: (thick(54), thick(50)),
            lower: (thick(36), thick(28)),
            foot: thick(31),
            paw,
            sides: 6,
        };
        for index in 0..4 {
            s.brush(SUIT_LEG, COAT, leg_bone(index, 0));
            let (l, look) = if index < 2 {
                (&sk.fore, &look_fore)
            } else {
                (&sk.hind, &look_hind)
            };
            leg(&mut s, l, index, look);
        }
        sheep::tail(&mut s, &sk, ONE, level < 2);
    } else {
        lean_body(&mut s, &coat);
        for index in 0..4 {
            wolf_leg(&mut s, &coat, if index < 2 { &sk.fore } else { &sk.hind }, index);
        }
        wolf_tail(&mut s, &coat, &sk);
    }

    s.flat = false;
    let disguised = Hiding {
        head_size: t.disguise_head,
        muzzle: t.disguise_muzzle,
        muzzle_wide: t.disguise_muzzle_wide,
        ears: t.disguise_ears,
        ..*t
    };
    if suit {
        head_wolf(&mut s, &sk, &disguised);
    } else {
        head_mean(&mut s, &sk, &coat);
    }

    s.stretch(cm(t.width), cm(t.length));
    s
}

/// Oval rings along the body, cm: (z, y, half width, half height).
fn oval_rings(list: &[(i32, i32, i32, i32)]) -> Vec<Ring> {
    list.iter()
        .map(|(z, y, wide, tall)| Ring {
            at: v3(0, cm(*y), cm(*z)),
            wide: cm(*wide),
            tall: cm(*tall),
        })
        .collect()
}

/// The plain Wolf's trunk: a real wolf's line. Deep at the chest, high at the
/// withers, a tucked belly, the loin narrow and the croup falling to the tail,
/// with a thick ruff round the neck and shoulders.
fn lean_body(s: &mut Shape, coat: &Coat) {
    let loin = [
        (-56, 64, 4, 5),
        (-48, 65, 12, 11),
        (-22, 66, 13, 11),
        (6, 65, 14, 14),
    ];
    let chest = [
        (-8, 65, 13, 13),
        (10, 61, 16, 17),
        (26, 60, 16, 18),
        (38, 67, 6, 6),
    ];
    let neck = [(21, 62, 8, 9), (33, 73, 14, 15), (40, 81, 10, 10), (43, 84, 5, 5)];
    let share = |p: V3| coat.share(p);
    let down = v3(0, -ONE * 3 / 10, 0);
    let nap = coat.nap(true, down, &share, 8, 17);
    s.brush([ONE; 3], COAT, BODY);
    pelt(s, &oval_rings(&loin), &Nap { capped: true, ..nap });
    s.bone = CHEST;
    pelt(s, &oval_rings(&chest), &Nap { seed: 7, ..nap });
    s.bone = NECK;
    pelt(s, &oval_rings(&neck), &Nap { seed: 27, ..nap });
}

/// The tail: thick and bushy, hanging straight down, a dark tip.
fn wolf_tail(s: &mut Shape, coat: &Coat, sk: &Skeleton) {
    let [a, b, c] = sk.tail;
    let tip = c.add(v3(0, -cm(10), -cm(2)));
    let share = |p: V3| lerp(cm(coat.t.back), MASK[0], ONE - smooth(cm(30), cm(44), p.y));
    let nap = coat.nap(false, V3::ZERO, &share, 4, 307);
    s.brush([ONE; 3], COAT, TAIL[0]);
    pelt(
        s,
        &[ring(a, cm(3)), ring(a.lerp(b, ONE / 2), cm(6)), ring(b, cm(6))],
        &nap,
    );
    s.bone = TAIL[1];
    pelt(
        s,
        &[
            ring(a.lerp(b, cm(70)), cm(4)),
            ring(b.lerp(c, ONE / 3), cm(7)),
            ring(c, cm(5)),
        ],
        &Nap { seed: 311, ..nap },
    );
    s.bone = TAIL[2];
    pelt(
        s,
        &[ring(b.lerp(c, cm(70)), cm(3)), ring(c, cm(5)), ring(tip, mm(8))],
        &Nap { seed: 313, ..nap },
    );
}

/// The disguise: the sheep's own soft wool in one egg over the whole trunk,
/// as fine as the sheep around it at each distance, and its neck ruff.
fn cloud_coat(s: &mut Shape, t: &Hiding, sk: &Skeleton, level: u32) {
    let wrap = sheep::WRAP[level as usize];
    // a deeper coat hangs lower and keeps its top where it was
    let deep = cm(t.depth) / 2;
    let radii = v3(cm(t.puff), cm(SUIT_SIZE.0) + deep, cm(SUIT_SIZE.1));
    let center = v3(0, cm(SUIT_AT.0) - deep, cm(SUIT_AT.1));
    let cluster = lumps(V3::ZERO, radii, cm(t.shag), t.clumps, 7);
    s.flat = false;
    s.brush([ONE; 3], COAT, BODY);
    sheep::puff(s, center, radii, &cluster, mm(sheep::RISE), wrap);
    sheep::neck_wool(s, sk.neck.lerp(sk.head, cm(16)), cm(92), wrap - 1);
}

/// The plain Wolf's head, in the mm of its own frame: a long flat wedge from
/// the back of the skull straight to the nose in the same pelt as the rest, a
/// dark mask down the top of it, amber eyes slanted under a heavy brow and
/// left clear of the fur, big upright ears, the jaw a little open on two
/// teeth.
fn head_mean(s: &mut Shape, sk: &Skeleton, coat: &Coat) {
    let (t, near) = (coat.t, coat.level < 2);
    let (fwd, up) = head_frame(deg(6));
    let size = cm(t.head_size);
    let mm = |n: i32| mul(mm(n), size);
    let reach = |n: i32| 40 + (n - 40) * t.muzzle / 100;
    let thick = |n: i32| mm(n * t.muzzle_wide / 100);
    let ear = |n: i32| n * t.ears / 100;
    let side = v3(ONE, 0, 0);
    let o = sk.head;
    let h = |along: i32, high: i32, x: i32| {
        o.add(fwd.scale(mm(along)))
            .add(up.scale(mm(high)))
            .add(side.scale(mm(x)))
    };
    let oval = |along: i32, high: i32, wide: i32, tall: i32| Ring {
        at: h(along, high, 0),
        wide,
        tall,
    };
    let (pale, lip) = (cm(t.face), cm(t.face - 12));
    // where a point is along the head and how high on it
    let frame = |p: V3| (p.sub(o).dot(fwd), p.sub(o).dot(up));
    let share = |p: V3| {
        let (along, high) = frame(p);
        let nose = smooth(mm(46), mm(reach(240)), along);
        let cut = lerp(mm(30), mm(-12), nose) + lerp(mm(50), thick(26), nose) * 55 / 100;
        lerp(pale, MASK[0], smooth(cut - mm(8), cut + mm(8), high))
    };
    // no tuft lies over an eye, or the Wolf turns into a Sheepdog
    let open = |p: V3| {
        let (along, high) = frame(p);
        !((mm(-10)..mm(150)).contains(&along) && high > mm(10) && p.sub(o).dot(side).abs() > mm(14))
    };
    let close = Nap {
        blur: mm(4),
        open: &open,
        ..coat.nap(true, V3::ZERO, &share, 6, 401)
    };
    s.brush([ONE; 3], COAT, HEAD);
    // no dome and no stop: the top line runs straight from poll to nose
    pelt(
        s,
        &[
            oval(-70, 26, mm(48), mm(52)),
            oval(-20, 32, mm(84), mm(62)),
            oval(46, 30, mm(66), mm(50)),
            oval(reach(110), 14, thick(52), thick(42)),
            oval(reach(190), -4, thick(42), thick(34)),
            oval(reach(262), -16, thick(26), thick(18)),
        ],
        &Nap {
            capped: true,
            ..close
        },
    );
    // pale cheeks that flare: tufts sweeping back and down off the skull
    for x in [-1, 1] {
        let cheek = |high: i32, down: i32, seed: i32| {
            let hair = fwd
                .neg()
                .add(up.scale(-ONE * down / 100))
                .add(side.scale(x * ONE / 4))
                .norm();
            let n = side.scale(x * ONE).add(up.scale(-ONE / 3)).norm();
            (h(26, high, x * 56), n, hair, seed)
        };
        for (p, n, hair, seed) in [cheek(10, 10, 1), cheek(-16, 45, 2), cheek(-34, 70, 3)] {
            tuft(
                s,
                p,
                n,
                hair,
                coat.tuft,
                mul(pale, ONE * 24 / 25 + noise(seed * x + 409) / 25),
            );
        }
    }
    s.brush(NOSE, OWN, HEAD);
    s.spike(h(reach(232), -4, 0), h(reach(274), -8, 0), thick(24), 4, up);
    for x in [-1, 1] {
        // the dark line round the eye, the amber, the pupil, flat on the
        // facet, each a little proud of the last; the outer corner sits high
        let n = side
            .scale(x * ONE * 3 / 4)
            .add(up.scale(ONE * 11 / 20))
            .add(fwd.scale(ONE * 7 / 20))
            .norm();
        let corner = fwd.neg().add(up.scale(ONE * 2 / 5));
        let u = corner.sub(n.scale(corner.dot(n))).norm();
        let v = n.cross(u).norm();
        let center = h(64, 50, x * 54);
        s.paint = OWN;
        let mut diamond = |color: Rgb, long: i32, wide: i32, out: i32| {
            let c = center.add(n.scale(mm(out)));
            let (a, b) = (c.add(u.scale(mm(long))), c.sub(u.scale(mm(long))));
            let (l, r) = (c.add(v.scale(mm(wide))), c.sub(v.scale(mm(wide))));
            s.color = color;
            s.face(a, l, b, center.sub(n));
            s.face(a, b, r, center.sub(n));
        };
        diamond(PUPIL, 32, 14, 6);
        diamond(WOLF_EYE, 24, 9, 8);
        if near {
            diamond(PUPIL, 6, 6, 10);
        }
        // the brow comes down hard toward the nose
        s.brush(MASK, COAT, HEAD);
        let (inner, outer) = (h(92, 56, x * 20), h(26, 78, x * 74));
        let (ridge, under) = (h(58, 84, x * 44), h(60, 62, x * 56));
        let inside = h(58, 40, 0);
        s.face(inner, ridge, under, inside);
        s.face(ridge, outer, under, inside);
    }
    // the open mouth's dark, then the lower jaw hanging clear of it
    s.brush(PUPIL, OWN, HEAD);
    s.tube(
        &[
            oval(60, -28, thick(44), thick(18)),
            oval(reach(205), -36, thick(26), thick(16)),
        ],
        4,
        side,
    );
    if near {
        s.brush(TOOTH, OWN, HEAD);
        for x in [-1, 1] {
            s.spike(
                h(reach(216), -26, x * 21),
                h(reach(216), -56, x * 19),
                mm(8),
                3,
                fwd,
            );
        }
    }
    let lip_share = |_: V3| lip;
    s.brush([ONE; 3], COAT, JAW);
    pelt(
        s,
        &[
            oval(30, -38, thick(46), thick(20)),
            oval(reach(212), -62, thick(27), thick(10)),
        ],
        &Nap {
            share: &lip_share,
            sides: coat.sides(4),
            capped: true,
            seed: 431,
            ..close
        },
    );
    // big, upright, set forward, dark behind and pale inside
    let back_share = |_: V3| MASK[0];
    let ear_nap = Nap {
        backward: false,
        share: &back_share,
        sides: coat.sides(4),
        ..close
    };
    for (x, bone) in [(1, EAR_LEFT), (-1, EAR_RIGHT)] {
        let e = |out: i32, high: i32, ahead: i32| h(ahead + 4, 74 + ear(high), x * (46 + ear(out)));
        let leaf = |at: V3, wide: i32, tall: i32| Ring {
            at,
            wide: mm(ear(wide)),
            tall: mm(ear(tall)),
        };
        s.brush([ONE; 3], COAT, bone);
        pelt(
            s,
            &[
                leaf(e(0, -8, 0), 48, 20),
                leaf(e(10, 60, -6), 36, 14),
                leaf(e(18, 124, -10), 6, 4),
            ],
            &Nap {
                seed: 443 + x,
                ..ear_nap
            },
        );
        // the pale inside: one facet on the ear's front
        s.color = [pale * 4 / 5; 3];
        let inside = e(0, 40, -40);
        s.face(e(-22, 8, 26), e(26, 8, 26), e(16, 96, 8), inside);
    }
}

/// The disguise's head: a short soft snout and small pointed ears.
fn head_wolf(s: &mut Shape, sk: &Skeleton, t: &Hiding) {
    let (fwd, up) = head_frame(deg(10));
    let (pale, lip, dark) = (cm(t.face), cm(t.face + 8), mm(760));
    // every length below scales with the head
    let size = cm(t.head_size);
    let mm = |n: i32| mul(mm(n), size);
    let cm = |n: i32| mul(cm(n), size);
    // the muzzle from the skull out, then how wide it is, and the ears' size
    let reach = |n: i32| 40 + (n - 40) * t.muzzle / 100;
    let thick = |n: i32| mm(n * t.muzzle_wide / 100);
    let ear = |n: i32| n * t.ears / 100;
    let side = v3(ONE, 0, 0);
    let o = sk.head;
    let h = |along: i32, high: i32, x: i32| {
        o.add(fwd.scale(mm(along)))
            .add(up.scale(mm(high)))
            .add(side.scale(mm(x)))
    };
    s.patch = None;
    s.brush([ONE; 3], COAT, HEAD);
    s.ball(h(-10, 34, 0), v3(mm(104), mm(88), mm(112)), &Affine::IDENTITY, 1);
    // a long muzzle, pale, tapering square to the nose
    s.brush([pale; 3], COAT, HEAD);
    s.tube(
        &[
            Ring {
                at: h(40, 10, 0),
                wide: thick(66),
                tall: thick(58),
            },
            Ring {
                at: h(reach(110), -6, 0),
                wide: thick(50),
                tall: thick(42),
            },
            Ring {
                at: h(reach(190), -14, 0),
                wide: thick(42),
                tall: thick(34),
            },
            Ring {
                at: h(reach(240), -18, 0),
                wide: thick(32),
                tall: thick(26),
            },
        ],
        8,
        side,
    );
    // pale cheeks that flare, as the photographs show
    s.brush([pale; 3], COAT, HEAD);
    for x in [-1, 1] {
        s.ball(
            h(-14, -24, x * 66),
            v3(cm(3), cm(4), cm(6)),
            &Affine::rot_x(deg(30)),
            1,
        );
    }
    s.brush(NOSE, OWN, HEAD);
    s.ball(
        h(reach(248), -8, 0),
        v3(thick(24), thick(20), thick(20)),
        &Affine::IDENTITY,
        1,
    );
    for x in [-50, 50] {
        s.brush(WOLF_EYE, OWN, HEAD);
        s.ball(h(58, 58, x), v3(mm(14), mm(12), mm(13)), &Affine::IDENTITY, 1);
        s.brush(PUPIL, OWN, HEAD);
        s.ball(
            h(64, 59, x * 112 / 100),
            v3(mm(7), mm(7), mm(7)),
            &Affine::IDENTITY,
            1,
        );
    }
    // the lower jaw, with a little dark along the lip
    s.brush([lip; 3], COAT, JAW);
    s.tube(
        &[
            Ring {
                at: h(70, -40, 0),
                wide: thick(50),
                tall: thick(20),
            },
            Ring {
                at: h(reach(210), -50, 0),
                wide: thick(28),
                tall: thick(10),
            },
        ],
        6,
        side,
    );
    // erect, pointed ears, dark at the back
    for (x, bone) in [(1, EAR_LEFT), (-1, EAR_RIGHT)] {
        s.brush([dark; 3], COAT, bone);
        let e = |out: i32, high: i32, ahead: i32| h(ahead - 20, 82 + ear(high), x * (58 + ear(out)));
        s.tube(
            &[
                Ring {
                    at: e(0, 0, 0),
                    wide: mm(ear(40)),
                    tall: mm(ear(18)),
                },
                Ring {
                    at: e(8, 56, -4),
                    wide: mm(ear(30)),
                    tall: mm(ear(12)),
                },
                Ring {
                    at: e(14, 104, -6),
                    wide: mm(ear(6)),
                    tall: mm(ear(4)),
                },
            ],
            6,
            side,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::hiding::HIDING;
    use crate::models::rig::leg_bone;

    #[test]
    fn a_wolfs_hind_leg_is_deep_at_the_thigh_and_narrow_at_the_cannon() {
        let shape = wolf(&HIDING, false, 0);
        let (vertices, _) = shape.finish();
        let depth = |bone: i32, lo: i32, hi: i32| {
            let zs = || {
                vertices
                    .iter()
                    .filter(|v| v.bone == bone && v.pos.y > cm(lo) && v.pos.y < cm(hi))
                    .map(|v| v.pos.z)
            };
            zs().max().unwrap() - zs().min().unwrap()
        };
        // the thigh's skin is 2 x 104 mm front to back; the cannon's 2 x 19 mm
        // and its tufts' fall inside the slice
        let thigh = depth(leg_bone(2, 0), 54, 58);
        let cannon = depth(leg_bone(2, 2), 4, 26);
        assert!(thigh > cm(20), "thigh {}", thigh * 100 / ONE);
        assert!(cannon < cm(9), "cannon {}", cannon * 100 / ONE);
    }

    /// The mean edge of a mesh's triangles on some bones.
    fn mean_edge(bones: &[i32]) -> i32 {
        let (vertices, indices) = wolf(&HIDING, false, 0).finish();
        let (mut sum, mut count) = (0i64, 0i64);
        for t in indices.chunks(3) {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| vertices[i as usize]);
            if bones.contains(&a.bone) {
                sum += (a.pos.dist(b.pos) + b.pos.dist(c.pos) + c.pos.dist(a.pos)) as i64;
                count += 3;
            }
        }
        (sum / count) as i32
    }

    #[test]
    fn a_wolf_is_about_eleven_hundred_triangles() {
        // a tenth of the 11,254 it had, give or take a fifth
        let near = wolf(&HIDING, false, 0).triangles();
        assert!((880..=1320).contains(&near), "{near}");
    }

    #[test]
    fn a_wolfs_face_is_as_coarse_as_its_body() {
        // a tuft is 13 cm long on either, so the mean edge differs only
        // by the head's small parts: eyes, nose, teeth
        let (head, body) = (mean_edge(&[HEAD]), mean_edge(&[BODY, CHEST]));
        assert!(head * 2 > body, "head {head}, body {body}");
    }

    #[test]
    fn nothing_grows_over_a_wolfs_eyes() {
        // the eye as `head_mean` places it at the table's sizes
        let sk = rig::rest(Rig::Wolf);
        let (fwd, up) = head_frame(deg(6));
        let side = v3(ONE, 0, 0);
        let eye = sk
            .head
            .add(fwd.scale(mm(64)))
            .add(up.scale(mm(50)))
            .add(side.scale(mm(54)));
        let out = side
            .scale(ONE * 3 / 4)
            .add(up.scale(ONE * 11 / 20))
            .add(fwd.scale(ONE * 7 / 20))
            .norm();
        // seen straight down on the eye, its center is the origin
        let flat = up.sub(out.scale(up.dot(out))).norm();
        let across = out.cross(flat).norm();
        let (vertices, indices) = wolf(&HIDING, false, 0).finish();
        let mut seen = 0;
        for f in indices.chunks(3) {
            let [a, b, c] = [f[0], f[1], f[2]].map(|i| vertices[i as usize]);
            if a.bone != HEAD || a.paint != COAT {
                continue;
            }
            let on = |p: &crate::mesh::Vertex| {
                let d = p.pos.sub(eye);
                (d.dot(flat) as i64, d.dot(across) as i64, d.dot(out))
            };
            let [p, q, r] = [on(&a), on(&b), on(&c)];
            seen += 1;
            // proud of the eye by more than the 8 mm it stands
            if (p.2 + q.2 + r.2) / 3 <= cm(1) {
                continue;
            }
            // the eye's middle and four points about its edge
            let reach = mm(20) as i64;
            for (u, w) in [(0, 0), (reach, 0), (-reach, 0), (0, reach), (0, -reach)] {
                let turn = |s: (i64, i64, i32), e: (i64, i64, i32)| {
                    (e.0 - s.0) * (w - s.1) - (e.1 - s.1) * (u - s.0)
                };
                let turns = [turn(p, q), turn(q, r), turn(r, p)];
                let over = turns.iter().all(|t| *t > 0) || turns.iter().all(|t| *t < 0);
                assert!(!over, "{:?} {:?} {:?}", a.pos, b.pos, c.pos);
            }
        }
        assert!(seen > 50, "the fur was looked at: {seen}");
    }
}
