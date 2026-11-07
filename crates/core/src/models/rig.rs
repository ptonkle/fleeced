//! Skeletons and movement: what makes an animal walk, built once as a table.
//!
//! Every animal has the same list of bones, so the pose table is one float
//! texture. A row of the table is one moment of one clip: for each bone, the
//! matrix that takes the model from its rest pose to that moment. The vertex
//! shader reads two rows and blends them, so moving a sheep costs the core a
//! row number and the page nothing. [`Walker`] is what a scene keeps per
//! animal: its clip, its phase, and the blend out of the clip before.
//!
//! Legs are solved, not keyed: each clip says where the feet go, and a
//! two-bone solve (hip and knee, with the foot's own tip) finds the joints.
//! Feet in stance slide back at exactly the body's speed, so they plant.
//! [`stride`] says how far one cycle travels, for that to hold in play.

use crate::fx::{self, div, mul, sin, ONE, TURN};
use crate::render::geom::{atan2, cm, deg, mm, smooth, v3, Affine, V3};

use super::hiding::{Hiding, HIDING};

pub const BONE_COUNT: usize = 23;
/// A bone's matrix takes three texels of the pose texture.
pub const TEXELS_PER_BONE: i32 = 3;
/// The pose texture's width in texels.
pub const POSE_WIDTH: i32 = BONE_COUNT as i32 * TEXELS_PER_BONE;

pub const BODY: i32 = 0;
pub const CHEST: i32 = 1;
pub const NECK: i32 = 2;
pub const HEAD: i32 = 3;
pub const EAR_LEFT: i32 = 4;
pub const EAR_RIGHT: i32 = 5;
pub const JAW: i32 = 6;
pub const TAIL: [i32; 3] = [7, 8, 9];
pub const BELL: i32 = 10;
const FIRST_LEG: i32 = 11;

/// A leg's bones: `leg` 0 fore left, 1 fore right, 2 hind left, 3 hind
/// right; `part` 0 upper, 1 lower, 2 foot.
pub const fn leg_bone(leg: usize, part: usize) -> i32 {
    FIRST_LEG + (leg * 3 + part) as i32
}

/// The skeletons. A ram is a big ewe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rig {
    Sheep,
    Lamb,
    Dog,
    Wolf,
}

impl Rig {
    pub const ALL: [Rig; 4] = [Rig::Sheep, Rig::Lamb, Rig::Dog, Rig::Wolf];
}

/// What an animal can be doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Clip {
    Idle,
    /// a sheep's head down and chewing; a canine's nose along the ground
    Graze,
    Walk,
    Run,
    Crouch,
    /// crouched and moving
    Stalk,
    /// grazing along: short slow steps, a sheep's head still down, a canine
    /// crouched
    Drift,
    /// from the take-off to the landing; it holds its last row
    Jump,
    /// a bark, a bite: the head thrust forward, the mouth open
    Snap,
    /// the forepaws digging
    Dig,
    /// the muzzle lifted to the sky
    Howl,
    /// a win: a Sheepdog bounces, barking, its tail going; a Wolf howls, over
    /// and over
    Cheer,
    /// a win: a Wolf's head low over what it ate, ears back, teeth showing
    Gloat,
}

impl Clip {
    pub const ALL: [Clip; 13] = [
        Clip::Idle,
        Clip::Graze,
        Clip::Walk,
        Clip::Run,
        Clip::Crouch,
        Clip::Stalk,
        Clip::Drift,
        Clip::Jump,
        Clip::Snap,
        Clip::Dig,
        Clip::Howl,
        Clip::Cheer,
        Clip::Gloat,
    ];

    /// Rows of the table one cycle spans.
    pub const fn steps(self) -> i32 {
        match self {
            Clip::Idle | Clip::Crouch | Clip::Graze => 12,
            Clip::Jump | Clip::Snap | Clip::Dig | Clip::Howl => 12,
            Clip::Walk | Clip::Run | Clip::Stalk | Clip::Drift => 16,
            Clip::Cheer | Clip::Gloat => 24,
        }
    }

    /// Whether the clip repeats; a clip that does not holds its end.
    pub const fn loops(self) -> bool {
        !matches!(self, Clip::Jump | Clip::Snap | Clip::Howl)
    }

    /// Ticks one cycle of a clip takes when feet do not set its pace.
    pub const fn ticks(self) -> i32 {
        match self {
            Clip::Idle | Clip::Crouch => 240,
            Clip::Graze => 150,
            Clip::Snap => 18,
            Clip::Dig => 24,
            Clip::Howl => 180,
            Clip::Jump => 40,
            Clip::Walk | Clip::Run | Clip::Stalk | Clip::Drift => 60,
            Clip::Cheer => 44,
            Clip::Gloat => 200,
        }
    }

    /// The moment of a win clip a still scene shows: a Sheepdog on the
    /// ground, a Wolf's muzzle at the top of its howl.
    pub const fn held(self, rig: Rig) -> i32 {
        match (self, rig) {
            (Clip::Cheer, Rig::Wolf) => ONE / 2,
            _ => 0,
        }
    }
}

/// Meters one cycle of a moving clip covers, so feet plant at any speed.
pub fn stride(rig: Rig, clip: Clip) -> i32 {
    match (rig, clip) {
        (Rig::Sheep, Clip::Walk) => cm(95),
        (Rig::Sheep, Clip::Run) => cm(170),
        (Rig::Sheep, Clip::Stalk) => cm(70),
        (Rig::Sheep, Clip::Drift) => cm(46),
        (Rig::Lamb, Clip::Walk) => cm(65),
        (Rig::Lamb, Clip::Run) => cm(130),
        (Rig::Lamb, Clip::Stalk) => cm(50),
        (Rig::Lamb, Clip::Drift) => cm(34),
        (Rig::Dog, Clip::Walk) => cm(125),
        (Rig::Dog, Clip::Run) => cm(250),
        (Rig::Dog, Clip::Stalk) => cm(85),
        (Rig::Dog, Clip::Drift) => cm(46),
        (Rig::Wolf, Clip::Walk) => cm(150),
        (Rig::Wolf, Clip::Run) => cm(290),
        (Rig::Wolf, Clip::Stalk) => cm(95),
        (Rig::Wolf, Clip::Drift) => cm(46),
        _ => ONE,
    }
}

/// One leg at rest, in the side view: (z, y) of each joint, and how far it
/// stands from the middle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Leg {
    pub hip: (i32, i32),
    pub knee: (i32, i32),
    pub ankle: (i32, i32),
    pub foot: (i32, i32),
    pub x: i32,
}

/// Where a body's joints are at rest. The meshes are built on these same
/// points, so a model and its movement agree by construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Skeleton {
    pub body: V3,
    pub chest: V3,
    pub neck: V3,
    pub head: V3,
    pub ear: V3,
    pub jaw: V3,
    pub tail: [V3; 3],
    pub bell: V3,
    pub fore: Leg,
    pub hind: Leg,
}

const fn yz(y: i32, z: i32) -> V3 {
    v3(0, y, z)
}

/// An animal's joints as the hiding table sets them.
pub fn skeleton(rig: Rig) -> Skeleton {
    fitted(&HIDING, rig)
}

/// An animal's joints with a table's width and length. A Wolf is built on
/// [`rest`] and stretched the same way, so model and movement still agree.
pub fn fitted(h: &Hiding, rig: Rig) -> Skeleton {
    let sk = rest(rig);
    if rig != Rig::Wolf {
        return sk;
    }
    let (wide, long) = (cm(h.width), cm(h.length));
    let p = |a: V3| v3(mul(a.x, wide), a.y, mul(a.z, long));
    let joint = |a: (i32, i32)| (mul(a.0, long), a.1);
    let leg = |l: Leg| Leg {
        hip: joint(l.hip),
        knee: joint(l.knee),
        ankle: joint(l.ankle),
        foot: joint(l.foot),
        x: mul(l.x, wide),
    };
    Skeleton {
        body: p(sk.body),
        chest: p(sk.chest),
        neck: p(sk.neck),
        head: p(sk.head),
        ear: p(sk.ear),
        jaw: p(sk.jaw),
        tail: sk.tail.map(p),
        bell: p(sk.bell),
        fore: leg(sk.fore),
        hind: leg(sk.hind),
    }
}

/// The joints before the hiding table has its say.
pub fn rest(rig: Rig) -> Skeleton {
    match rig {
        // length 1.05 m, shoulder 0.62 m: a round barrel on slim legs
        Rig::Sheep => Skeleton {
            body: yz(cm(42), -cm(2)),
            chest: yz(cm(44), cm(8)),
            neck: yz(cm(47), cm(28)),
            head: yz(cm(64), cm(41)),
            ear: v3(cm(5), cm(68), cm(44)),
            jaw: yz(cm(57), cm(48)),
            tail: [yz(cm(50), -cm(41)), yz(cm(44), -cm(44)), yz(cm(38), -cm(45))],
            bell: yz(cm(44), cm(43)),
            fore: Leg {
                hip: (cm(25), cm(38)),
                knee: (cm(23), cm(24)),
                ankle: (cm(25), cm(11)),
                foot: (cm(26), 0),
                x: cm(12),
            },
            hind: Leg {
                hip: (-cm(25), cm(40)),
                knee: (-cm(19), cm(27)),
                ankle: (-cm(31), cm(15)),
                foot: (-cm(28), 0),
                x: cm(12),
            },
        },
        // length 0.60 m, shoulder 0.38 m: long legs, a big head
        Rig::Lamb => Skeleton {
            body: yz(cm(29), 0),
            chest: yz(cm(30), cm(6)),
            neck: yz(cm(31), cm(14)),
            head: yz(cm(42), cm(20)),
            ear: v3(cm(4), cm(46), cm(22)),
            jaw: yz(cm(37), cm(25)),
            tail: [yz(cm(34), -cm(22)), yz(cm(29), -cm(26)), yz(cm(24), -cm(28))],
            bell: yz(cm(30), cm(24)),
            fore: Leg {
                hip: (cm(12), cm(26)),
                knee: (cm(11), cm(17)),
                ankle: (cm(12), cm(8)),
                foot: (cm(13), 0),
                x: cm(7),
            },
            hind: Leg {
                hip: (-cm(12), cm(28)),
                knee: (-cm(8), cm(19)),
                ankle: (-cm(16), cm(10)),
                foot: (-cm(14), 0),
                x: cm(7),
            },
        },
        // length 0.95 m, shoulder 0.56 m: a border collie
        Rig::Dog => Skeleton {
            body: yz(cm(43), -cm(8)),
            chest: yz(cm(44), cm(2)),
            neck: yz(cm(47), cm(19)),
            head: yz(cm(61), cm(30)),
            ear: v3(cm(5), cm(68), cm(29)),
            jaw: yz(cm(56), cm(35)),
            tail: [yz(cm(46), -cm(37)), yz(cm(33), -cm(44)), yz(cm(21), -cm(47))],
            bell: yz(cm(47), cm(28)),
            fore: Leg {
                hip: (cm(18), cm(40)),
                knee: (cm(13), cm(27)),
                ankle: (cm(16), cm(10)),
                foot: (cm(18), 0),
                x: cm(9),
            },
            hind: Leg {
                hip: (-cm(29), cm(42)),
                knee: (-cm(22), cm(29)),
                ankle: (-cm(35), cm(16)),
                foot: (-cm(32), 0),
                x: cm(9),
            },
        },
        // length 1.25 m, shoulder 0.80 m: long legs, a deep chest
        Rig::Wolf => Skeleton {
            body: yz(cm(63), -cm(12)),
            chest: yz(cm(64), cm(6)),
            neck: yz(cm(68), cm(28)),
            head: yz(cm(83), cm(43)),
            ear: v3(cm(6), cm(92), cm(42)),
            jaw: yz(cm(77), cm(50)),
            tail: [yz(cm(66), -cm(51)), yz(cm(50), -cm(58)), yz(cm(32), -cm(60))],
            bell: yz(cm(67), cm(38)),
            fore: Leg {
                hip: (cm(26), cm(58)),
                knee: (cm(20), cm(40)),
                ankle: (cm(24), cm(15)),
                foot: (cm(26), 0),
                x: cm(11),
            },
            hind: Leg {
                hip: (-cm(42), cm(62)),
                knee: (-cm(33), cm(42)),
                ankle: (-cm(50), cm(22)),
                foot: (-cm(46), 0),
                x: cm(11),
            },
        },
    }
}

/// One foot's target this moment: how far from its rest point, and how far
/// its last bone tips back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Foot {
    dz: i32,
    dy: i32,
    tip: i32,
}

/// One moment of a body, before the joints are solved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Stance {
    drop: i32,
    shift: i32,
    pitch: i32,
    flex: i32,
    /// how far the feet come in under the body
    gather: i32,
    neck: i32,
    neck_yaw: i32,
    head: i32,
    ear: i32,
    jaw: i32,
    tail: [i32; 3],
    wag: i32,
    bell: i32,
    feet: [Foot; 4],
}

/// sin of `cycles` turns through the phase `t`, shifted by `at` of a turn.
fn wave(t: i32, cycles: i32, at: i32) -> i32 {
    sin(t.wrapping_mul(cycles) + at)
}

/// Where a foot is in a gait: `start` is the fraction of the cycle its
/// stance begins, `duty` how much of the cycle it stands, `travel` how far
/// it slides back while it does, `lift` how high it swings.
fn foot(t: i32, start: i32, duty: i32, travel: i32, lift: i32) -> Foot {
    let u = (t - start).rem_euclid(TURN);
    if u < duty {
        let k = div(u, duty);
        Foot {
            dz: mul(travel, ONE / 2 - k),
            dy: 0,
            tip: mul(deg(30), smooth(cm(55), ONE, k)),
        }
    } else {
        let k = div(u - duty, ONE - duty);
        Foot {
            dz: mul(travel, smooth(0, ONE, k) - ONE / 2),
            dy: mul(lift, sin(k / 2)),
            tip: mul(deg(30), sin(k / 2 + TURN / 6).max(0)) + mul(deg(36), sin(k / 2)),
        }
    }
}

/// A gait's feet: `starts` for fore left, fore right, hind left, hind right.
fn gait(t: i32, starts: [i32; 4], duty: i32, length: i32, lift: i32) -> [Foot; 4] {
    let travel = mul(length, duty);
    starts.map(|s| foot(t, s, duty, travel, lift))
}

/// Fractions of a cycle, in hundredths.
const fn c(n: i32) -> i32 {
    cm(n)
}

/// One foot at a time, for the slow gaits.
const SLOW_FEET: [i32; 4] = [c(25), c(75), 0, c(50)];

/// A standing Wolf's shoulder, cm; the crouch sinks to the table's height.
const WOLF_SHOULDER: i32 = 80;

/// A body's size, for scaling a clip's bounce to the animal.
fn scale_of(rig: Rig) -> i32 {
    match rig {
        Rig::Sheep => ONE,
        Rig::Lamb => cm(62),
        Rig::Dog => cm(90),
        Rig::Wolf => cm(128),
    }
}

fn stance(h: &Hiding, rig: Rig, clip: Clip, t: i32) -> Stance {
    let k = scale_of(rig);
    let canine = matches!(rig, Rig::Dog | Rig::Wolf);
    let at = |n: i32| mul(mm(n), k);
    let mut s = Stance::default();
    if canine {
        // a canine's tail hangs in a low curve unless it has a reason
        s.tail = [deg(4), deg(10), deg(8)];
    }
    match clip {
        Clip::Idle => {
            s.drop = mul(at(3), wave(t, 1, 0));
            s.neck = mul(deg(3), wave(t, 1, TURN / 3));
            s.neck_yaw = mul(deg(9), wave(t, 1, 0));
            s.head = mul(deg(3), wave(t, 2, 0));
            s.ear = mul(deg(5), wave(t, 3, 0));
            s.wag = mul(deg(if canine { 12 } else { 4 }), wave(t, 4, 0));
        }
        Clip::Graze => {
            // the neck reaches down and the mouth works at the grass
            let chew = wave(t, 3, 0);
            s.pitch = deg(7);
            s.drop = at(45);
            s.neck = if canine { deg(64) } else { deg(105) };
            // the head turns back up on the neck, so the face stays in front
            // and the mouth reaches the grass
            s.head = if canine { deg(6) } else { -deg(74) } + mul(deg(4), chew);
            s.jaw = mul(deg(6), chew.max(0));
            s.neck_yaw = mul(deg(10), wave(t, 1, 0));
            s.ear = mul(deg(6), wave(t, 1, TURN / 4));
            s.feet[0].dz = at(20);
            s.feet[1].dz = -at(20);
            s.wag = mul(deg(6), wave(t, 2, 0));
        }
        Clip::Walk => {
            let (starts, duty, lift) = if canine {
                // a trot: the diagonals move together
                ([0, c(50), c(50), 0], c(33), at(70))
            } else {
                // a walk: one foot at a time, a little quick for a trot
                ([c(25), c(75), 0, c(50)], c(40), at(60))
            };
            s.feet = gait(t, starts, duty, stride(rig, clip), lift);
            s.drop = mul(at(if canine { 12 } else { 9 }), wave(t, 2, TURN / 4));
            s.pitch = mul(deg(1), wave(t, 2, 0));
            s.neck = deg(if canine { 10 } else { 3 }) + mul(deg(3), wave(t, 2, TURN / 8));
            s.head = -deg(if canine { 8 } else { 2 });
            s.neck_yaw = mul(deg(3), wave(t, 1, 0));
            s.ear = mul(deg(8), wave(t, 2, TURN / 3));
            s.wag = mul(deg(if canine { 16 } else { 8 }), wave(t, 1, 0));
            s.tail[1] += if canine { -deg(8) } else { 0 };
            s.bell = mul(deg(16), wave(t, 2, TURN / 6));
        }
        Clip::Run => {
            // a gallop: the hind pair lands, then the fore pair, with the
            // spine folding and opening between them
            let (starts, duty, lift) = if canine {
                ([c(52), c(62), 0, c(9)], c(18), at(120))
            } else {
                ([c(50), c(60), 0, c(10)], c(20), at(110))
            };
            s.feet = gait(t, starts, duty, stride(rig, clip), lift);
            let reach = wave(t, 1, -TURN / 8);
            s.drop = -mul(at(if canine { 34 } else { 24 }), wave(t, 2, -TURN / 8));
            s.pitch = mul(deg(if canine { 6 } else { 5 }), wave(t, 1, TURN / 8));
            s.flex = mul(deg(if canine { 11 } else { 4 }), reach);
            s.neck = deg(if canine { 20 } else { 8 }) + mul(deg(6), reach);
            s.head = -deg(14);
            s.ear = deg(26);
            if canine {
                s.tail = [deg(52), deg(6), -deg(4)];
                s.tail[1] += mul(deg(14), wave(t, 1, TURN / 2));
            } else {
                s.tail = [deg(30), 0, 0];
            }
            s.bell = mul(deg(26), wave(t, 2, TURN / 4));
        }
        Clip::Drift if !canine => {
            let chew = wave(t, 3, 0);
            s.pitch = deg(6);
            s.drop = at(35);
            s.neck = deg(100);
            s.head = -deg(70) + mul(deg(4), chew);
            s.jaw = mul(deg(6), chew.max(0));
            s.neck_yaw = mul(deg(8), wave(t, 1, 0));
            s.ear = mul(deg(6), wave(t, 1, TURN / 4));
            s.feet = gait(t, SLOW_FEET, c(70), stride(rig, clip), at(30));
        }
        Clip::Crouch | Clip::Stalk | Clip::Drift => {
            let breathe = wave(t, 1, 0);
            s.neck_yaw = mul(deg(5), wave(t, 1, TURN / 3));
            s.ear = deg(if canine { 28 } else { 18 });
            match rig {
                // the collie's eye: head out level, below the shoulders
                Rig::Dog => {
                    s.drop = cm(18);
                    s.pitch = deg(3);
                    s.neck = deg(44);
                    s.head = -deg(32);
                    s.tail = [deg(30), deg(20), deg(6)];
                }
                // down to a ewe's height, the head low, the tail tucked
                // against the rump: from behind it reads as a round back.
                // Every number is in the hiding table.
                Rig::Wolf => {
                    s.drop = cm(WOLF_SHOULDER - h.height);
                    s.pitch = deg(h.pitch);
                    s.flex = deg(h.arch);
                    s.neck = deg(h.neck);
                    s.head = deg(h.head);
                    s.tail = [deg(h.tail), deg(6), deg(4)];
                    s.ear = deg(h.ears_back);
                    s.gather = cm(h.gather);
                }
                // a flinch: legs bent, the head drawn in
                _ => {
                    s.drop = at(70);
                    s.neck = -deg(10);
                    s.head = deg(18);
                    s.tail = [deg(20), 0, 0];
                }
            }
            match clip {
                Clip::Stalk => {
                    s.feet = gait(t, SLOW_FEET, c(50), stride(rig, clip), at(55));
                    s.drop += mul(at(8), wave(t, 2, TURN / 4));
                    s.neck_yaw = mul(deg(3), wave(t, 1, 0));
                }
                Clip::Drift => {
                    s.feet = gait(t, SLOW_FEET, c(70), stride(rig, clip), at(30));
                    s.drop += mul(at(3), wave(t, 2, TURN / 4));
                    s.neck_yaw = mul(deg(3), wave(t, 1, 0));
                }
                _ => {
                    s.drop += mul(at(3), breathe);
                    // the feet settle a little wider apart
                    s.feet[0].dz = at(36);
                    s.feet[1].dz = at(36);
                    s.feet[2].dz = -at(24);
                    s.feet[3].dz = -at(24);
                }
            }
            // the fore feet go with a chest that slides back
            s.feet[0].dz -= s.gather;
            s.feet[1].dz -= s.gather;
            s.feet[2].dz += s.gather;
            s.feet[3].dz += s.gather;
        }
        Clip::Jump => {
            // gather and spring, tuck in the air, reach for the ground
            let spring = smooth(0, c(24), t) - smooth(c(24), c(44), t);
            let tuck = smooth(c(18), c(48), t) - smooth(c(62), c(88), t);
            let reach = smooth(c(58), c(84), t) - smooth(c(92), ONE, t);
            s.pitch = mul(-deg(18), spring) + mul(deg(13), reach);
            s.neck = mul(deg(14), reach) - mul(deg(10), spring);
            s.head = mul(-deg(6), reach);
            s.ear = mul(deg(22), tuck);
            let up = at(190);
            for f in 0..2 {
                s.feet[f] = Foot {
                    dz: mul(at(-40), tuck) + mul(at(110), reach),
                    dy: mul(up, tuck) + mul(at(60), spring),
                    tip: mul(deg(50), tuck),
                };
            }
            for f in 2..4 {
                s.feet[f] = Foot {
                    dz: mul(at(70), tuck) - mul(at(90), spring),
                    dy: mul(up, tuck),
                    tip: mul(deg(44), tuck) + mul(deg(22), spring),
                };
            }
            s.tail = [mul(-deg(24), tuck) + s.tail[0], s.tail[1], s.tail[2]];
            s.bell = mul(deg(30), wave(t, 2, 0));
        }
        Clip::Snap => {
            let out = smooth(0, c(30), t) - smooth(c(55), ONE, t);
            s.neck = mul(-deg(10), out);
            s.head = mul(-deg(14), out);
            s.jaw = mul(deg(30), out);
            s.pitch = mul(deg(4), out);
            s.drop = mul(at(14), out);
            s.tail[0] -= mul(deg(14), out);
            s.feet[0].dz = at(30);
        }
        Clip::Dig => {
            // both forepaws scrape in turn, the head down over them
            s.pitch = deg(9);
            s.drop = at(46);
            s.neck = deg(38);
            s.head = -deg(8);
            s.tail = [-deg(26), -deg(6), 0];
            s.wag = mul(deg(22), wave(t, 2, 0));
            for f in 0..2 {
                let u = wave(t, 1, TURN * f as i32 / 2);
                s.feet[f] = Foot {
                    dz: mul(at(70), u),
                    dy: at(40) + mul(at(40), wave(t, 1, TURN * f as i32 / 2 + TURN / 4)),
                    tip: mul(deg(20), u.max(0)),
                };
            }
        }
        Clip::Howl => {
            let up = smooth(0, c(30), t) - smooth(c(82), ONE, t);
            s.neck = mul(-deg(56), up);
            s.head = mul(-deg(30), up);
            s.jaw = mul(deg(16), up);
            s.drop = mul(at(24), up);
            s.pitch = mul(-deg(7), up);
            s.ear = mul(deg(24), up);
            s.tail[0] += mul(deg(12), up);
        }
        Clip::Cheer if rig == Rig::Wolf => {
            // the same howl, held long and let go, and again
            let up = smooth(c(8), c(26), t) - smooth(c(76), c(94), t);
            s.neck = mul(-deg(58), up);
            s.head = mul(-deg(30), up);
            s.jaw = mul(deg(18) + mul(deg(4), wave(t, 7, 0)), up);
            s.drop = mul(at(24), up);
            s.pitch = mul(-deg(8), up);
            s.ear = mul(deg(24), up);
            s.tail[0] += mul(deg(12), up);
        }
        Clip::Cheer => {
            // up off all four feet and down into a crouch, barking at the top
            let air = wave(t, 1, 0).max(0);
            let squat = (-wave(t, 1, 0)).max(0);
            s.drop = mul(at(70), squat) - mul(at(190), air);
            s.pitch = mul(-deg(15), air) + mul(deg(5), squat);
            s.neck = -deg(16) - mul(deg(14), air);
            s.head = mul(-deg(8), air);
            s.jaw = mul(deg(26), air);
            s.ear = mul(deg(16), wave(t, 2, 0));
            s.tail = [-deg(52), -deg(6), deg(4)];
            s.wag = mul(deg(44), wave(t, 3, 0));
            for f in 0..4 {
                let lift = if f < 2 { at(250) } else { at(190) };
                s.feet[f].dy = mul(lift, air);
                s.feet[f].tip = mul(deg(30), air);
            }
        }
        Clip::Gloat => {
            let slow = wave(t, 1, 0);
            s.pitch = deg(6);
            s.drop = at(52);
            s.neck = deg(40);
            // the head turns back up, so the eyes stay on the camera
            s.head = -deg(36) + mul(deg(3), wave(t, 2, 0));
            s.neck_yaw = mul(deg(15), slow);
            s.jaw = deg(7) + mul(deg(5), wave(t, 3, 0).max(0));
            s.ear = deg(30);
            s.tail = [-deg(18), deg(2), deg(2)];
            s.wag = mul(deg(5), wave(t, 2, 0));
        }
    }
    if rig == Rig::Wolf && matches!(clip, Clip::Idle | Clip::Walk | Clip::Run) {
        s.neck += deg(h.carry);
        s.head -= deg(h.carry);
    }
    s
}

/// Turns a point in the side view by `a` about the x axis, as `rot_x` does.
fn turned(p: (i32, i32), a: i32) -> (i32, i32) {
    let (s, co) = (sin(a), fx::cos(a));
    (mul(s, p.1) + mul(co, p.0), mul(co, p.1) - mul(s, p.0))
}

fn sub(a: (i32, i32), b: (i32, i32)) -> (i32, i32) {
    (a.0 - b.0, a.1 - b.1)
}

fn long(p: (i32, i32)) -> i32 {
    fx::length(p.0, p.1)
}

/// The rotation about x that takes the direction `from` to `to`.
fn angle_between(from: (i32, i32), to: (i32, i32)) -> i32 {
    atan2(from.1, from.0) - atan2(to.1, to.0)
}

/// The two-bone solve in the side view: where the knee goes for a hip and an
/// ankle, bending to the side `side` says.
fn solve(hip: (i32, i32), ankle: (i32, i32), upper: i32, lower: i32, side: i32) -> (i32, i32) {
    let to = sub(ankle, hip);
    let reach = long(to).clamp((upper - lower).abs() + mm(2), upper + lower - mm(2));
    let dir = {
        let l = long(to).max(1);
        (div(to.0, l), div(to.1, l))
    };
    let along = div(
        mul(upper, upper) + mul(reach, reach) - mul(lower, lower),
        2 * reach,
    );
    let across = fx::sqrt(mul(upper, upper) - mul(along, along));
    (
        hip.0 + mul(dir.0, along) - mul(dir.1, mul(across, side)),
        hip.1 + mul(dir.1, along) + mul(dir.0, mul(across, side)),
    )
}

fn posed(leg: &Leg, parent: &Affine, foot: Foot, out: &mut [Affine], bones: [i32; 3]) {
    let at = |p: (i32, i32)| v3(leg.x, p.1, p.0);
    let hip3 = parent.apply(v3(0, leg.hip.1, leg.hip.0));
    let hip = (hip3.z, hip3.y);
    let rest_foot_bone = sub(leg.foot, leg.ankle);
    let toe = (leg.foot.0 + foot.dz, leg.foot.1 + foot.dy);
    let ankle = sub(toe, turned(rest_foot_bone, foot.tip));
    let (upper, lower) = (long(sub(leg.knee, leg.hip)), long(sub(leg.ankle, leg.knee)));
    // which side the rest knee lies on, seen from the hip along the leg
    let (a, b) = (sub(leg.ankle, leg.hip), sub(leg.knee, leg.hip));
    let side = if mul(a.0, b.1) - mul(a.1, b.0) >= 0 {
        ONE
    } else {
        -ONE
    };
    let knee = solve(hip, ankle, upper, lower, side);
    let ankle_at = {
        // if the ankle was out of reach, the leg ends where it can
        let to = sub(ankle, knee);
        let l = long(to).max(1);
        (
            knee.0 + mul(div(to.0, l), lower),
            knee.1 + mul(div(to.1, l), lower),
        )
    };
    let parts = [
        (leg.hip, leg.knee, hip, knee),
        (leg.knee, leg.ankle, knee, ankle_at),
        (leg.ankle, leg.foot, ankle_at, toe),
    ];
    for (k, (rest_a, rest_b, now_a, now_b)) in parts.iter().enumerate() {
        let turn = Affine::rot_x(angle_between(sub(*rest_b, *rest_a), sub(*now_b, *now_a)));
        out[bones[k] as usize] = Affine::translate(at(*now_a))
            .then(&turn)
            .then(&Affine::translate(at(*rest_a).neg()));
    }
}

/// Every bone's matrix for one moment.
pub fn pose(rig: Rig, clip: Clip, t: i32) -> [Affine; BONE_COUNT] {
    pose_with(&HIDING, rig, clip, t)
}

/// Every bone's matrix for one moment, with a hiding table of its own.
pub fn pose_with(h: &Hiding, rig: Rig, clip: Clip, t: i32) -> [Affine; BONE_COUNT] {
    let sk = fitted(h, rig);
    let s = stance(h, rig, clip, t);
    let mut m = [Affine::IDENTITY; BONE_COUNT];
    let body =
        Affine::translate(v3(0, -s.drop, s.shift)).then(&Affine::about(sk.body, &Affine::rot_x(s.pitch)));
    let chest = body.then(&Affine::about(sk.chest, &Affine::rot_x(s.flex)));
    let turn = Affine::rot_y(s.neck_yaw).then(&Affine::rot_x(s.neck));
    let neck = chest.then(&Affine::about(sk.neck, &turn));
    let head = neck.then(&Affine::about(sk.head, &Affine::rot_x(s.head)));
    m[BODY as usize] = body;
    m[CHEST as usize] = chest;
    m[NECK as usize] = neck;
    m[HEAD as usize] = head;
    let ear_left = v3(sk.ear.x, sk.ear.y, sk.ear.z);
    let ear_right = v3(-sk.ear.x, sk.ear.y, sk.ear.z);
    // a canine's ears stand up and lie back; a sheep's stick out and droop
    let (turn_left, turn_right) = if matches!(rig, Rig::Dog | Rig::Wolf) {
        (Affine::rot_x(-s.ear), Affine::rot_x(-s.ear))
    } else {
        (Affine::rot_z(-s.ear), Affine::rot_z(s.ear))
    };
    m[EAR_LEFT as usize] = head.then(&Affine::about(ear_left, &turn_left));
    m[EAR_RIGHT as usize] = head.then(&Affine::about(ear_right, &turn_right));
    m[JAW as usize] = head.then(&Affine::about(sk.jaw, &Affine::rot_x(s.jaw)));
    let mut tail = body;
    for k in 0..3 {
        let bend = if k == 0 {
            Affine::rot_y(s.wag).then(&Affine::rot_x(s.tail[0]))
        } else {
            Affine::rot_y(s.wag / 2).then(&Affine::rot_x(s.tail[k]))
        };
        tail = tail.then(&Affine::about(sk.tail[k], &bend));
        m[TAIL[k] as usize] = tail;
    }
    m[BELL as usize] = neck.then(&Affine::about(sk.bell, &Affine::rot_x(s.bell)));
    for leg in 0..4 {
        let (mut shape, parent) = if leg < 2 {
            (sk.fore, chest)
        } else {
            (sk.hind, body)
        };
        if leg % 2 == 1 {
            shape.x = -shape.x;
        }
        let bones = [leg_bone(leg, 0), leg_bone(leg, 1), leg_bone(leg, 2)];
        posed(&shape, &parent, s.feet[leg], &mut m, bones);
    }
    m
}

/// The pose table as the page uploads it, and where each clip starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoseTable {
    /// four values a texel, `POSE_WIDTH` texels a row
    pub values: Vec<i32>,
    pub rows: i32,
    first: Vec<i32>,
}

impl PoseTable {
    pub fn build() -> PoseTable {
        PoseTable::build_with(&HIDING)
    }

    /// Row 0 is the still row: every bone at rest, for what has no bones.
    pub fn build_with(h: &Hiding) -> PoseTable {
        let mut t = PoseTable {
            values: Vec::new(),
            rows: 0,
            first: Vec::new(),
        };
        t.push_row(&[Affine::IDENTITY; BONE_COUNT]);
        for rig in Rig::ALL {
            for clip in Clip::ALL {
                t.first.push(t.rows);
                // one more row than steps: a blend across the end of a loop
                // reads the first moment again
                for step in 0..=clip.steps() {
                    let phase = fx::ratio(step, clip.steps());
                    let phase = if clip.loops() { phase % ONE } else { phase };
                    t.push_row(&pose_with(h, rig, clip, phase));
                }
            }
        }
        t
    }

    fn push_row(&mut self, bones: &[Affine; BONE_COUNT]) {
        for b in bones {
            self.values.extend_from_slice(&[
                b.c0.x, b.c1.x, b.c2.x, b.t.x, b.c0.y, b.c1.y, b.c2.y, b.t.y, b.c0.z, b.c1.z, b.c2.z, b.t.z,
            ]);
        }
        self.rows += 1;
    }

    fn slot(rig: Rig, clip: Clip) -> usize {
        let r = Rig::ALL.iter().position(|x| *x == rig).unwrap_or(0);
        let c = Clip::ALL.iter().position(|x| *x == clip).unwrap_or(0);
        r * Clip::ALL.len() + c
    }

    /// The row, with its fraction, of a clip at phase `phase`.
    pub fn row(&self, rig: Rig, clip: Clip, phase: i32) -> i32 {
        let p = if clip.loops() {
            phase.rem_euclid(ONE)
        } else {
            phase.clamp(0, ONE)
        };
        fx::from_int(self.first[Self::slot(rig, clip)]) + p * clip.steps()
    }
}

/// The game's rate (`tune::TICKS_PER_SECOND` once the sim lands it).
const TICKS_PER_SECOND: i32 = 60;
/// Ticks a change of clip takes to blend.
const BLEND_TICKS: i32 = 10;

/// What a scene keeps per animal: what it is doing, and what it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Walker {
    pub clip: Clip,
    pub phase: i32,
    was: Clip,
    was_phase: i32,
    /// how much of the old clip still shows
    blend: i32,
}

impl Walker {
    pub const fn new(clip: Clip, phase: i32) -> Walker {
        Walker {
            clip,
            phase,
            was: clip,
            was_phase: phase,
            blend: 0,
        }
    }

    /// One tick: moving at `speed` meters a second, wanting clip `want`.
    pub fn step(&mut self, rig: Rig, want: Clip, speed: i32) {
        if want != self.clip {
            self.was = self.clip;
            self.was_phase = self.phase;
            self.clip = want;
            self.blend = ONE;
            self.phase = 0;
        }
        self.phase += advance(rig, self.clip, speed);
        self.was_phase += advance(rig, self.was, speed);
        self.blend = (self.blend - ONE / BLEND_TICKS).max(0);
    }

    /// The pose the page reads: two rows and the blend between them.
    pub fn pose(&self, rig: Rig, table: &PoseTable) -> [i32; 3] {
        [
            table.row(rig, self.clip, self.phase),
            table.row(rig, self.was, self.was_phase),
            self.blend,
        ]
    }
}

/// How far through its cycle a clip moves in one tick.
fn advance(rig: Rig, clip: Clip, speed: i32) -> i32 {
    match clip {
        Clip::Walk | Clip::Run | Clip::Stalk | Clip::Drift => {
            div(speed, stride(rig, clip)) / TICKS_PER_SECOND
        }
        _ => ONE / clip.ticks(),
    }
}
