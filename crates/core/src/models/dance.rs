//! The winners dance: a Wolf or a Sheepdog up on its hind legs like a person.
//!
//! One file, on purpose: this is a /try. `ON` set to false puts the winners
//! back on `Clip::Cheer` and `Clip::Gloat`; the rig only calls [`pose`] and
//! lists [`Dance::ALL`].
//!
//! `rig.rs` works in the side view, because an animal's legs only swing fore
//! and aft. A person's arms do not, so a dance has its own pose: every joint
//! takes a rotation in the world, not in its parent, and a hanging arm is
//! simply no rotation at all. A [`Moves`] is one moment, written as a person
//! stands: `spread` takes an arm out through the side, `raise` forward. Hind
//! feet are planted points and a three-dimensional solve finds the leg.
//!
//! The body turns about the hips, which move over the spot it stood on, so
//! `Clip::Rise` is the same pose with `up` going from none to all.

use crate::fx::{self, mul, ONE, TURN};
use crate::render::geom::{atan2, cm, deg, lerp, mm, smooth, v3, Affine, V3};

use super::hiding::Hiding;
use super::rig::{
    angle_between, fitted, leg_bone, long, scale_of, solve, sub, Clip, Leg, Rig, Walker, BELL, BODY,
    BONE_COUNT, CHEST, EAR_LEFT, EAR_RIGHT, HEAD, JAW, NECK, TAIL,
};

/// Whether winners dance. False is the whole revert of this try.
pub const ON: bool = true;

/// How far the body tips back from four feet to two. Short of straight
/// up, so the weight stays over the feet and the chest shows.
const LEAN: i32 = deg(80);
/// How far back of the neck's rest it comes, so the head still looks ahead.
const NECK_UP: i32 = deg(14);
/// How far the tail swings out behind, past hanging down.
const TAIL_OUT: i32 = deg(58);
/// How far the hips always sit below full height, so a leg has reach left.
const SIT: i32 = mm(28);

/// How far each shoulder moves out from where the forelegs hang, so a
/// raised or hanging arm clears the coat instead of going through it. A
/// Wolf's shaggy coat stands out well past its forelegs.
const fn shoulder_out(rig: Rig) -> i32 {
    match rig {
        Rig::Wolf => cm(9),
        _ => cm(4),
    }
}

/// Rows of the pose table a dance takes.
pub const DANCE_STEPS: i32 = 40;
pub const RISE_STEPS: i32 = 16;
/// Ticks getting up takes, and how far apart the seats start.
pub const RISE_TICKS: i32 = 55;
const RISE_STAGGER: u32 = 14;
/// Ticks each dance lasts before a winner moves on to its next one, and
/// the ticks that change takes.
const SPELL: u32 = 540;
const CHANGE: u32 = 16;
/// How far along [`ORDER`] a winner moves between spells: more than one, so
/// a seat never lands on the dance its neighbor is still finishing.
const SKIP: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Dance {
    Disco,
    Robot,
    Twist,
    RunningMan,
    Sprinkler,
    Letters,
    Floss,
    Chicken,
}

impl Dance {
    /// In the order seats take them: fast next to slow, arms up next to
    /// arms down, so neighbors look unlike each other.
    pub const ALL: [Dance; 8] = [
        Dance::Disco,
        Dance::Robot,
        Dance::Floss,
        Dance::Letters,
        Dance::RunningMan,
        Dance::Sprinkler,
        Dance::Twist,
        Dance::Chicken,
    ];

    /// Ticks one cycle takes.
    pub const fn ticks(self) -> i32 {
        match self {
            Dance::Disco | Dance::Twist => 88,
            Dance::RunningMan => 64,
            Dance::Floss => 56,
            Dance::Robot | Dance::Sprinkler => 168,
            Dance::Letters | Dance::Chicken => 192,
        }
    }

    /// The moment a still scene holds.
    pub const fn held(self) -> i32 {
        match self {
            Dance::Robot => ONE / 3,
            Dance::Sprinkler => ONE / 2,
            Dance::Letters => ONE * 7 / 8,
            _ => ONE / 8,
        }
    }
}

/// A winner's dance on this spell. Wolves start further along, so the two
/// teams open differently, and a lone winner still gets through all eight.
pub fn dance_of(rig: Rig, k: i32, spell: u32) -> Dance {
    let start = if rig == Rig::Wolf { 3 } else { 0 };
    let at = k as u32 + start + spell * SKIP;
    Dance::ALL[(at % Dance::ALL.len() as u32) as usize]
}

/// What a winner in seat `k` is doing `ticks` into the screen; `None` is a
/// still scene, which holds a dance from the first.
pub fn walker(rig: Rig, k: i32, ticks: Option<u32>) -> Walker {
    let Some(ticks) = ticks else {
        let dance = dance_of(rig, k, 0);
        return Walker::new(Clip::Dance(dance), dance.held());
    };
    let danced = ticks.saturating_sub(k as u32 * RISE_STAGGER);
    if danced < RISE_TICKS as u32 {
        return Walker::new(Clip::Rise, fx::ratio(danced as i32, RISE_TICKS));
    }
    let danced = danced - RISE_TICKS as u32;
    let (spell, into) = (danced / SPELL, danced % SPELL);
    let clip = Clip::Dance(dance_of(rig, k, spell));
    let phase = dance_phase(clip, k, danced);
    if into >= CHANGE {
        return Walker::new(clip, phase);
    }
    let (was, was_phase) = match spell.checked_sub(1) {
        Some(last) => {
            let was = Clip::Dance(dance_of(rig, k, last));
            (was, dance_phase(was, k, danced))
        }
        None => (Clip::Rise, ONE),
    };
    Walker::blended(
        clip,
        phase,
        was,
        was_phase,
        ONE - fx::ratio(into as i32, CHANGE as i32),
    )
}

/// Each seat has its own length of cycle, so no two keep time.
fn dance_phase(clip: Clip, k: i32, ticks: u32) -> i32 {
    let cycle = (clip.ticks() + k * clip.ticks() / 9) as u32;
    fx::ratio((ticks % cycle) as i32, cycle as i32)
}

/// An arm: `spread` out through the side (90 straight out, 180 overhead),
/// `raise` forward from there, and the forearm's `elbow` forward and `curl`
/// on through the side. Each arm mirrors the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Arm {
    spread: i32,
    raise: i32,
    elbow: i32,
    curl: i32,
}

/// An arm in degrees: spread, raise, elbow, curl.
const fn arm(spread: i32, raise: i32, elbow: i32, curl: i32) -> Arm {
    Arm {
        spread: deg(spread),
        raise: deg(raise),
        elbow: deg(elbow),
        curl: deg(curl),
    }
}

impl Arm {
    fn to(self, o: Arm, k: i32) -> Arm {
        Arm {
            spread: lerp(self.spread, o.spread, k),
            raise: lerp(self.raise, o.raise, k),
            elbow: lerp(self.elbow, o.elbow, k),
            curl: lerp(self.curl, o.curl, k),
        }
    }
}

/// Where a hind foot is from where it stands: to the left, ahead, up; and
/// how far it swivels on the spot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Step {
    side: i32,
    ahead: i32,
    lift: i32,
    turn: i32,
}

impl Step {
    fn to(self, o: Step, k: i32) -> Step {
        Step {
            side: lerp(self.side, o.side, k),
            ahead: lerp(self.ahead, o.ahead, k),
            lift: lerp(self.lift, o.lift, k),
            turn: lerp(self.turn, o.turn, k),
        }
    }
}

/// One moment of a dance. Lengths are for a body the size of a sheep, and
/// the rig's own scale takes them to a Wolf or a Sheepdog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Moves {
    /// hips to the left, and down
    sway: i32,
    bob: i32,
    /// the hips turn and tip
    yaw: i32,
    roll: i32,
    /// the chest on the hips: turned, tipped sideways, bowed forward
    twist: i32,
    lean: i32,
    bow: i32,
    /// the head: turned, tipped sideways, nodded down
    look: i32,
    tilt: i32,
    nod: i32,
    jaw: i32,
    ear: i32,
    wag: i32,
    arms: [Arm; 2],
    feet: [Step; 2],
}

impl Moves {
    fn to(self, o: Moves, k: i32) -> Moves {
        let l = |a: i32, b: i32| lerp(a, b, k);
        Moves {
            sway: l(self.sway, o.sway),
            bob: l(self.bob, o.bob),
            yaw: l(self.yaw, o.yaw),
            roll: l(self.roll, o.roll),
            twist: l(self.twist, o.twist),
            lean: l(self.lean, o.lean),
            bow: l(self.bow, o.bow),
            look: l(self.look, o.look),
            tilt: l(self.tilt, o.tilt),
            nod: l(self.nod, o.nod),
            jaw: l(self.jaw, o.jaw),
            ear: l(self.ear, o.ear),
            wag: l(self.wag, o.wag),
            arms: [self.arms[0].to(o.arms[0], k), self.arms[1].to(o.arms[1], k)],
            feet: [self.feet[0].to(o.feet[0], k), self.feet[1].to(o.feet[1], k)],
        }
    }

    /// Arms swinging fore and aft against each other.
    fn swung(mut self, by: i32) -> Moves {
        self.arms[0].raise += by;
        self.arms[1].raise -= by;
        self
    }
}

/// sin of `n` cycles through `t`, shifted by `at` of a turn.
fn beat(t: i32, n: i32, at: i32) -> i32 {
    fx::sin(t.wrapping_mul(n) + at)
}

/// Fractions of a cycle, in hundredths.
const fn c(n: i32) -> i32 {
    cm(n)
}

/// Moves from key to key, snapping in the first `snap` of each beat and
/// holding the rest.
fn keyed(t: i32, keys: &[Moves], snap: i32) -> Moves {
    let at = t.rem_euclid(ONE) * keys.len() as i32;
    let (i, into) = ((at / ONE) as usize, at % ONE);
    keys[i % keys.len()].to(keys[(i + 1) % keys.len()], smooth(0, snap, into))
}

/// One arm up to the sky and down across the body, the other on its hip,
/// the hips rocking away from the point.
fn disco(t: i32) -> Moves {
    let u = beat(t, 2, 0);
    let high = ONE / 2 + u / 2;
    let step = beat(t, 2, TURN / 4).max(0);
    let point = Arm {
        spread: lerp(-deg(22), deg(150), high),
        raise: lerp(deg(26), deg(12), high),
        elbow: deg(4),
        curl: 0,
    };
    Moves {
        sway: mul(mm(85), u),
        bob: mm(30) + mul(mm(30), beat(t, 4, TURN / 4)),
        yaw: mul(deg(13), u),
        roll: mul(-deg(7), u),
        lean: mul(deg(9), u),
        look: mul(-deg(22), u),
        nod: mul(-deg(12), u),
        jaw: deg(9),
        wag: mul(deg(38), beat(t, 4, 0)),
        arms: [arm(36, -8, 0, -92), point],
        feet: [
            Step::default(),
            Step {
                side: mul(-mm(90), step),
                lift: mul(mm(40), step),
                turn: mul(-deg(24), step),
                ..Step::default()
            },
        ],
        ..Moves::default()
    }
}

/// Square shapes, one at a time, with a dead stop on each.
fn robot(t: i32) -> Moves {
    const SQUARE: Arm = arm(88, 0, 0, 92);
    const DOWN: Arm = arm(88, 0, 0, -88);
    let keys = [
        Moves {
            arms: [SQUARE, SQUARE],
            ..Moves::default()
        },
        Moves {
            twist: deg(26),
            look: deg(40),
            arms: [SQUARE, DOWN],
            ..Moves::default()
        },
        Moves {
            nod: deg(10),
            arms: [arm(0, 88, 0, 0), arm(0, 88, 0, 0)],
            ..Moves::default()
        },
        Moves {
            twist: -deg(26),
            look: -deg(40),
            arms: [DOWN, SQUARE],
            ..Moves::default()
        },
        Moves {
            bob: mm(50),
            look: deg(30),
            arms: [arm(12, 0, 92, 0), arm(12, 0, 92, 0)],
            ..Moves::default()
        },
        Moves {
            bob: mm(50),
            look: -deg(30),
            nod: -deg(12),
            arms: [arm(10, 130, 0, 0), arm(10, -28, 30, 0)],
            ..Moves::default()
        },
    ];
    keyed(t, &keys, c(18))
}

/// Hips one way and shoulders the other, both feet swiveling on the spot,
/// sinking down and coming back up.
fn twist(t: i32) -> Moves {
    let u = beat(t, 2, 0);
    let swivel = Step {
        turn: mul(deg(34), u),
        ..Step::default()
    };
    Moves {
        bob: mm(120) + mul(mm(70), beat(t, 1, 0)),
        yaw: mul(deg(34), u),
        twist: mul(-deg(56), u),
        bow: deg(10),
        look: mul(deg(10), u),
        nod: deg(6),
        jaw: deg(6),
        wag: mul(-deg(34), u),
        arms: [arm(26, 12, 92, 0), arm(26, 12, 92, 0)],
        feet: [swivel, swivel],
        ..Moves::default()
    }
    .swung(mul(deg(30), u))
}

/// A foot sliding back along the ground, then whipped through the air to
/// the front: one of a running man's two.
fn running_foot(t: i32) -> Step {
    let u = t.rem_euclid(ONE);
    let (reach, lift) = (mm(135), mm(120));
    if u < ONE / 2 {
        Step {
            ahead: lerp(reach, -reach, u * 2),
            ..Step::default()
        }
    } else {
        let k = (u - ONE / 2) * 2;
        Step {
            ahead: lerp(-reach, reach, smooth(0, ONE, k)),
            lift: mul(lift, fx::sin(k / 2)),
            ..Step::default()
        }
    }
}

/// The feet run on the spot, each sliding back as the other knee comes up,
/// and the arms pump.
fn running_man(t: i32) -> Moves {
    let u = beat(t, 1, 0);
    Moves {
        bob: mm(48) + mul(mm(26), beat(t, 2, TURN / 4)),
        bow: deg(9),
        twist: mul(-deg(16), u),
        look: mul(deg(8), u),
        jaw: deg(12),
        ear: deg(14),
        wag: mul(deg(30), beat(t, 2, 0)),
        arms: [arm(16, 0, 96, 0), arm(16, 0, 96, 0)],
        feet: [running_foot(t), running_foot(t + ONE / 2)],
        ..Moves::default()
    }
    .swung(mul(deg(34), u))
}

/// One paw behind the head and the other arm straight out, turning in
/// clicks across the view and whipping back round.
fn sprinkler(t: i32) -> Moves {
    let click = |n: i32| {
        let turned = deg(n * 15 - 48);
        Moves {
            bob: mm(30 + 25 * (n % 2)),
            yaw: turned / 2,
            twist: turned / 2,
            look: turned / 3,
            jaw: deg(6),
            arms: [arm(150, 0, 0, 104), arm(72, 16, 4, 0)],
            feet: [
                Step::default(),
                Step {
                    side: -mm(30),
                    ahead: mm(100),
                    ..Step::default()
                },
            ],
            ..Moves::default()
        }
    };
    let keys = [
        click(0),
        click(1),
        click(2),
        click(3),
        click(4),
        click(5),
        click(6),
        click(3),
    ];
    keyed(t, &keys, c(30))
}

/// Four big letters made with the arms, a step out on each.
fn letters(t: i32) -> Moves {
    let wide = |side: i32| {
        [
            Step {
                side,
                ..Step::default()
            },
            Step {
                side: -side,
                ..Step::default()
            },
        ]
    };
    let letter = |left: Arm, right: Arm, side: i32, lean: i32, bob: i32| Moves {
        lean,
        bob,
        jaw: deg(14),
        ear: deg(10),
        arms: [left, right],
        feet: wide(side),
        ..Moves::default()
    };
    let keys = [
        letter(arm(150, 0, 0, 0), arm(150, 0, 0, 0), mm(60), 0, mm(20)),
        letter(arm(120, 0, 0, 112), arm(120, 0, 0, 112), 0, 0, mm(70)),
        letter(arm(142, 0, 0, 34), arm(62, 0, 0, 62), 0, deg(12), mm(30)),
        letter(arm(152, 0, 0, 42), arm(152, 0, 0, 42), mm(30), 0, mm(10)),
    ];
    let mut m = keyed(t, &keys, c(34));
    m.bob += mul(mm(16), beat(t, 8, 0).abs());
    m.wag = mul(deg(26), beat(t, 8, 0));
    m
}

/// Straight arms swinging one way past the body, one in front and one
/// behind, and the hips the other.
fn floss(t: i32) -> Moves {
    let u = beat(t, 2, 0);
    let out = mul(deg(42), u);
    // an arm crosses in only a little: a deep coat swallows one swung
    // further in behind the back
    let across = |spread: i32| Arm {
        spread: spread.max(-deg(14)),
        elbow: deg(6),
        ..Arm::default()
    };
    Moves {
        sway: mul(-mm(120), u),
        bob: mm(30),
        roll: mul(deg(10), u),
        lean: mul(-deg(14), u),
        look: mul(deg(10), u),
        jaw: deg(16),
        wag: mul(-deg(46), u),
        arms: [across(out), across(-out)],
        ..Moves::default()
    }
    .swung(mul(deg(36), beat(t, 2, TURN / 4)))
}

/// Elbows flapping with the paws tucked in, then down into a shake of the
/// tail with the jaw going like a beak.
fn chicken(t: i32) -> Moves {
    let low = smooth(c(46), c(54), t) - smooth(c(92), ONE, t);
    let flap = beat(t, 8, 0);
    let shake = beat(t, 12, 0);
    let wings = Arm {
        spread: deg(62) + mul(deg(26), flap),
        ..arm(0, 0, 34, -124)
    };
    let high = Moves {
        bob: mm(30) + mul(mm(26), flap.abs()),
        nod: mul(deg(16), beat(t, 8, TURN / 4)),
        arms: [wings, wings],
        ..Moves::default()
    };
    let down = Moves {
        sway: mul(mm(44), shake),
        bob: mm(130),
        roll: mul(deg(9), shake),
        bow: deg(30),
        nod: -deg(14),
        jaw: mul(deg(26), flap.max(0)),
        wag: mul(deg(50), shake),
        arms: [wings, wings],
        ..Moves::default()
    };
    high.to(down, low)
}

fn moves(dance: Dance, t: i32) -> Moves {
    match dance {
        Dance::Disco => disco(t),
        Dance::Robot => robot(t),
        Dance::Twist => twist(t),
        Dance::RunningMan => running_man(t),
        Dance::Sprinkler => sprinkler(t),
        Dance::Letters => letters(t),
        Dance::Floss => floss(t),
        Dance::Chicken => chicken(t),
    }
}

/// A turn in the world: pitched, then tipped sideways, then turned.
fn turning(yaw: i32, roll: i32, pitch: i32) -> Affine {
    Affine::rot_y(yaw)
        .then(&Affine::rot_z(roll))
        .then(&Affine::rot_x(pitch))
}

/// A bone that takes `turn` in the world, hinged where `parent` has carried
/// its joint `at`.
fn joint(parent: &Affine, at: V3, turn: &Affine) -> Affine {
    Affine::translate(parent.apply(at))
        .then(turn)
        .then(&Affine::translate(at.neg()))
}

/// A hind leg in three dimensions: the hip is wherever the body has it, the
/// toe a point on the ground. The two-bone solve runs in the leg's own
/// plane, swiveled by `yaw` and tipped out to reach the foot.
fn planted(leg: &Leg, hip: V3, toe: V3, yaw: i32, out: &mut [Affine], bones: [i32; 3]) {
    let at = |p: (i32, i32)| v3(leg.x, p.1, p.0);
    let swivel = Affine::rot_y(yaw);
    let toe = Affine::rot_y(-yaw).apply_dir(toe.sub(hip));
    let foot = sub(leg.foot, leg.ankle);
    let ankle = v3(toe.x, toe.y - foot.1, toe.z - foot.0);
    let tip = atan2(ankle.x, -ankle.y);
    let flat_ankle = (ankle.z, -fx::length(ankle.x, ankle.y));
    let (upper, lower) = (long(sub(leg.knee, leg.hip)), long(sub(leg.ankle, leg.knee)));
    let (a, b) = (sub(leg.ankle, leg.hip), sub(leg.knee, leg.hip));
    let side = if mul(a.0, b.1) - mul(a.1, b.0) >= 0 {
        ONE
    } else {
        -ONE
    };
    let knee = solve((0, 0), flat_ankle, upper, lower, side);
    let ankle_at = {
        let to = sub(flat_ankle, knee);
        let l = long(to).max(1);
        (
            knee.0 + mul(fx::div(to.0, l), lower),
            knee.1 + mul(fx::div(to.1, l), lower),
        )
    };
    let plane = swivel.then(&Affine::rot_z(tip));
    let world = |p: (i32, i32)| hip.add(plane.apply_dir(v3(0, p.1, p.0)));
    let parts = [
        (leg.hip, leg.knee, (0, 0), knee),
        (leg.knee, leg.ankle, knee, ankle_at),
    ];
    for (k, (rest_a, rest_b, a, b)) in parts.into_iter().enumerate() {
        let turn = plane.then(&Affine::rot_x(angle_between(sub(rest_b, rest_a), sub(b, a))));
        out[bones[k] as usize] = Affine::translate(world(a))
            .then(&turn)
            .then(&Affine::translate(at(rest_a).neg()));
    }
    // the foot stays flat whichever way the leg tips
    let flat = Affine::translate(world(ankle_at)).then(&swivel);
    out[bones[2] as usize] = flat.then(&Affine::translate(at(leg.ankle).neg()));
}

/// Every bone for one moment: `up` is how far the body has risen.
fn stood(h: &Hiding, rig: Rig, m: &Moves, up: i32) -> [Affine; BONE_COUNT] {
    let sk = fitted(h, rig);
    let k = scale_of(rig);
    let size = |n: i32| mul(n, k);
    let of = |n: i32| mul(n, up);
    let pelvis = v3(0, sk.hind.hip.1, sk.hind.hip.0);
    let over_spot = of(-pelvis.z);
    let now = v3(
        size(m.sway),
        pelvis.y - size(m.bob + of(SIT)),
        pelvis.z + over_spot,
    );
    let pitch = of(-LEAN);
    let mut out = [Affine::IDENTITY; BONE_COUNT];

    let body = Affine::translate(now)
        .then(&turning(m.yaw, m.roll, pitch))
        .then(&Affine::translate(pelvis.neg()));
    let (yaw, roll) = (m.yaw + m.twist, m.roll + m.lean);
    let chest = joint(&body, sk.chest, &turning(yaw, roll, pitch + m.bow));
    let neck_pitch = of(-NECK_UP) + m.bow / 2 + m.nod / 2;
    let neck = joint(&chest, sk.neck, &turning(yaw + m.look / 2, roll, neck_pitch));
    let head = joint(&neck, sk.head, &turning(yaw + m.look, roll + m.tilt, m.nod));
    out[BODY as usize] = body;
    out[CHEST as usize] = chest;
    out[NECK as usize] = neck;
    out[HEAD as usize] = head;
    out[BELL as usize] = neck;
    let ear = Affine::rot_x(-m.ear);
    out[EAR_LEFT as usize] = head.then(&Affine::about(sk.ear, &ear));
    let right = v3(-sk.ear.x, sk.ear.y, sk.ear.z);
    out[EAR_RIGHT as usize] = head.then(&Affine::about(right, &ear));
    out[JAW as usize] = head.then(&Affine::about(sk.jaw, &Affine::rot_x(m.jaw)));

    // the tail keeps its low curve and swings out behind the standing body
    let mut tail = body;
    for (j, bone) in TAIL.iter().enumerate() {
        let curve = [deg(4), deg(10), deg(8)][j];
        let behind = if j == 0 { of(LEAN + TAIL_OUT) } else { 0 };
        let wag = if j == 0 { m.wag } else { m.wag / 2 };
        let bend = Affine::rot_z(wag).then(&Affine::rot_x(curve + behind));
        tail = tail.then(&Affine::about(sk.tail[j], &bend));
        out[*bone as usize] = tail;
    }

    let trunk = Affine::rot_y(yaw).then(&Affine::rot_z(roll));
    for leg in 0..2 {
        let sign = if leg == 0 { ONE } else { -ONE };
        let shape = Leg {
            x: mul(sk.fore.x, sign),
            ..sk.fore
        };
        let a = Arm::default().to(m.arms[leg], up);
        let at = |p: (i32, i32)| v3(shape.x, p.1, p.0);
        let upper = trunk
            .then(&Affine::rot_x(-a.raise))
            .then(&Affine::rot_z(mul(a.spread, sign)));
        let lower = upper
            .then(&Affine::rot_z(mul(a.curl, sign)))
            .then(&Affine::rot_x(-a.elbow));
        let out_by = mul(mul(shoulder_out(rig), sign), up);
        let shoulder = chest.then(&Affine::translate(v3(out_by, 0, 0)));
        let top = joint(&shoulder, at(shape.hip), &upper);
        let low = joint(&top, at(shape.knee), &lower);
        out[leg_bone(leg, 0) as usize] = top;
        out[leg_bone(leg, 1) as usize] = low;
        out[leg_bone(leg, 2) as usize] = joint(&low, at(shape.ankle), &lower);
    }

    for leg in 0..2 {
        let sign = if leg == 0 { ONE } else { -ONE };
        let shape = Leg {
            x: mul(sk.hind.x, sign),
            ..sk.hind
        };
        let f = m.feet[leg];
        let hip = body.apply(v3(shape.x, shape.hip.1, shape.hip.0));
        let toe = v3(
            shape.x + size(f.side),
            shape.foot.1 + size(f.lift),
            shape.foot.0 + over_spot + size(f.ahead),
        );
        let bones = [leg_bone(leg + 2, 0), leg_bone(leg + 2, 1), leg_bone(leg + 2, 2)];
        planted(&shape, hip, toe, f.turn, &mut out, bones);
    }
    out
}

/// The bones for a clip this file poses, or `None` for one the rig does.
pub fn pose(h: &Hiding, rig: Rig, clip: Clip, t: i32) -> Option<[Affine; BONE_COUNT]> {
    match clip {
        Clip::Rise => Some(stood(h, rig, &Moves::default(), smooth(0, ONE, t))),
        Clip::Dance(d) => Some(stood(h, rig, &moves(d, t), ONE)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::hiding::HIDING;
    use super::super::rig::{pose as bones_at, PoseTable};
    use super::super::Model;
    use super::*;
    use crate::fx::from_int;

    const DANCERS: [(Model, Rig); 2] = [(Model::Wolf, Rig::Wolf), (Model::Sheepdog, Rig::Dog)];

    fn clips() -> Vec<Clip> {
        let mut all: Vec<Clip> = Dance::ALL.iter().map(|d| Clip::Dance(*d)).collect();
        all.push(Clip::Rise);
        all
    }

    /// The lowest point of a whole mesh, every vertex on its own bone.
    fn lowest(model: Model, bones: &[Affine; BONE_COUNT]) -> i32 {
        let (vs, _) = model.build(0, &HIDING).finish();
        let at = |v: &crate::mesh::Vertex| {
            bones[v.bone.clamp(0, BONE_COUNT as i32 - 1) as usize]
                .apply(v.pos)
                .y
        };
        vs.iter().map(at).min().unwrap_or(0)
    }

    #[test]
    fn no_clip_takes_a_dancer_through_the_ground_or_off_it() {
        for (model, rig) in DANCERS {
            let rest = lowest(model, &[Affine::IDENTITY; BONE_COUNT]);
            for clip in clips() {
                for step in 0..clip.steps() {
                    let low = lowest(model, &bones_at(rig, clip, fx::ratio(step, clip.steps())));
                    assert!(
                        low >= rest - cm(2),
                        "{clip:?} {rig:?} step {step}: {low} under {rest}"
                    );
                    assert!(
                        low <= rest + cm(4),
                        "{clip:?} {rig:?} step {step}: {low} off the ground"
                    );
                }
            }
        }
    }

    #[test]
    fn a_dancer_stands_up_on_its_hind_legs_over_its_spot() {
        for (_, rig) in DANCERS {
            let sk = fitted(&HIDING, rig);
            let bones = bones_at(rig, Clip::Rise, ONE);
            let hip = bones[BODY as usize].apply(v3(0, sk.hind.hip.1, sk.hind.hip.0));
            let top = bones[HEAD as usize].apply(sk.head);
            let paw = bones[leg_bone(0, 2) as usize].apply(v3(sk.fore.x, sk.fore.foot.1, sk.fore.foot.0));
            assert!(hip.z.abs() < mm(1), "{rig:?} hips {} over the spot", hip.z);
            assert!(
                top.y > hip.y + (sk.chest.z - sk.hind.hip.0),
                "{rig:?} head {} on hips {}",
                top.y,
                hip.y
            );
            assert!(top.z.abs() < cm(45), "{rig:?} head {} over the feet", top.z);
            assert!(paw.y > hip.y / 2, "{rig:?} forepaw {} off the ground", paw.y);
        }
    }

    #[test]
    fn rising_starts_from_the_animal_at_rest() {
        for (_, rig) in DANCERS {
            let sk = fitted(&HIDING, rig);
            let bones = bones_at(rig, Clip::Rise, 0);
            let (head, ear) = (
                bones[HEAD as usize].apply(sk.head),
                bones[EAR_LEFT as usize].apply(sk.ear),
            );
            assert!(head.dist(sk.head) < mm(2) && ear.dist(sk.ear) < mm(2), "{rig:?}");
            let paw = v3(sk.hind.x, sk.hind.foot.1, sk.hind.foot.0);
            assert!(
                bones[leg_bone(2, 2) as usize].apply(paw).dist(paw) < mm(3),
                "{rig:?}"
            );
        }
    }

    #[test]
    fn every_dance_comes_back_round_without_a_jump() {
        for (_, rig) in DANCERS {
            for d in Dance::ALL {
                let (a, b) = (
                    bones_at(rig, Clip::Dance(d), 0),
                    bones_at(rig, Clip::Dance(d), ONE - 1),
                );
                let sk = fitted(&HIDING, rig);
                for (x, y) in a.iter().zip(&b) {
                    assert!(x.apply(sk.head).dist(y.apply(sk.head)) < mm(4), "{d:?} {rig:?}");
                }
            }
        }
    }

    #[test]
    fn a_full_team_shows_many_dances_and_no_neighbors_match() {
        let dance_at = |rig, k, tick| match walker(rig, k, Some(tick)).clip {
            Clip::Dance(d) => Some(d),
            _ => None,
        };
        for (_, rig) in DANCERS {
            for tick in (150..6000).step_by(7) {
                let team: Vec<Option<Dance>> = (0..7).map(|k| dance_at(rig, k, tick)).collect();
                for pair in team.windows(2) {
                    assert!(
                        pair[0] != pair[1] || pair[0].is_none(),
                        "{rig:?} tick {tick}: {pair:?}"
                    );
                }
                let mut seen = team.clone();
                seen.sort();
                seen.dedup();
                assert!(seen.len() >= 3, "{rig:?} tick {tick}: {seen:?}");
            }
        }
    }

    #[test]
    fn a_lone_winner_gets_through_all_eight() {
        let mut seen: Vec<Dance> = (0..8).map(|spell| dance_of(Rig::Wolf, 0, spell)).collect();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), 8);
    }

    #[test]
    fn a_still_scene_holds_a_dance() {
        let w = walker(Rig::Wolf, 2, None);
        assert_eq!((w.clip, w.phase), (Clip::Dance(Dance::Sprinkler), ONE / 2));
    }

    #[test]
    fn dances_have_rows_for_dancers_and_none_for_sheep() {
        let table = PoseTable::build();
        let dance = Clip::Dance(Dance::Disco);
        assert_ne!(table.row(Rig::Wolf, dance, 0), table.row(Rig::Dog, dance, 0));
        assert!(table.row(Rig::Dog, dance, 0) > from_int(1));
        assert_eq!(table.row(Rig::Sheep, dance, 0), 0);
    }
}
