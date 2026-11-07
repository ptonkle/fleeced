//! Every number that shapes the map, in one table, so a balance change
//! touches this file and nothing else. Meters unless a line says otherwise.

use crate::fx::{Fx, ONE};

/// A length given in tenths of a meter.
pub const fn tenths(n: i32) -> Fx {
    (n * ONE) / 10
}

/// A whole number of meters.
pub const fn meters(n: i32) -> Fx {
    n * ONE
}

// The hillside. x runs up the hill, from the low end at 0 to the Fold's end.
pub const LENGTH: Fx = meters(325);
pub const WIDTH: Fx = meters(175);
/// How far terrain carries on past the map on each side, for the view.
pub const TERRAIN_MARGIN: Fx = meters(40);
/// The terrain grid's spacing is 4 m, as a power of two of the Fx scale.
pub const TERRAIN_CELL_SHIFT: u32 = 18;
/// Height gained from the low end to the Fold's end. A slope of about 9.2%:
/// jumps are tuned against it, so this moves with `LENGTH`.
pub const RISE: Fx = meters(30);
/// Rolling: each octave is a coarse lattice, as (spacing shift, tallest
/// height either way). 64 m and 32 m.
pub const ROLL_OCTAVES: [(u32, Fx); 2] = [(22, tenths(21)), (21, tenths(5))];
/// The rolling fades to a quarter near the Fold, so the Fold sits level.
pub const ROLL_FADE_FROM: Fx = meters(262);
pub const ROLL_FADE_TO: Fx = meters(296);

/// What changes with Map size. Trees, cairns and young groups are counts per
/// field; every field keeps at least four trees and cairns for its mounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SizeRow {
    pub length: Fx,
    pub width: Fx,
    pub trees: [usize; 3],
    pub cairns: [usize; 3],
    pub groves: [usize; 3],
    /// The two fields each tunnel joins.
    pub tunnels: [(usize, usize); 4],
}

/// Small, Medium and Big. Big is today's map.
pub const SIZES: [SizeRow; 3] = [
    SizeRow {
        length: meters(195),
        width: meters(105),
        trees: [3, 3, 3],
        cairns: [1, 1, 1],
        groves: [1, 1, 0],
        tunnels: [(0, 2), (0, 1), (1, 2), (0, 1)],
    },
    SizeRow {
        length: meters(244),
        width: meters(131),
        trees: [4, 4, 3],
        cairns: [2, 2, 2],
        groves: [2, 1, 1],
        tunnels: TUNNEL_FIELDS,
    },
    SizeRow {
        length: LENGTH,
        width: WIDTH,
        trees: TREE_QUOTA,
        cairns: CAIRN_QUOTA,
        groves: GROVE_QUOTA,
        tunnels: TUNNEL_FIELDS,
    },
];

// The outer wall: the same outline on every seed, each corner nudged.
// Listed counterclockwise from the low end. The Fold is the same 26 m deep
// on any size of map: it is sized for the flock, not the hillside.
pub const OUTER_OUTLINE: [(i32, i32); 26] = [
    (12, 18),
    (30, 4),
    (75, 0),
    (125, 1),
    (175, 0),
    (225, 1),
    (258, 5),
    (279, 20),
    (291, 42),
    (299, 65),
    (323, 63),
    (325, 88),
    (323, 112),
    (299, 110),
    (291, 133),
    (279, 155),
    (258, 170),
    (225, 174),
    (175, 175),
    (125, 174),
    (75, 175),
    (30, 171),
    (12, 157),
    (4, 125),
    (0, 88),
    (4, 50),
];
/// From the Fold's front wall to the back of the map.
pub const FOLD_DEPTH: Fx = meters(26);
/// The corners of `OUTER_OUTLINE` that outline the Fold, in order. The
/// first and last are the ends of the Fold's front wall.
pub const FOLD_FIRST_CORNER: usize = 9;
pub const FOLD_LAST_CORNER: usize = 13;
pub const OUTER_NUDGE: Fx = meters(2);
pub const FOLD_NUDGE: Fx = meters(1);

// Stone walls. `height` is above the ground at the wall. Inner walls are
// jump height for Sheepdogs and Wolves; the outer wall stops everyone at any
// height, whatever its own height is.
pub const WALL_HALF: Fx = tenths(5);
pub const OUTER_WALL_HEIGHT: Fx = tenths(15);
pub const INNER_WALL_HEIGHT: Fx = tenths(9);
pub const FOLD_WALL_HEIGHT: Fx = tenths(9);

// The two inner walls that make three fields.
pub const DIVIDER_X: [Fx; 2] = [meters(110), meters(212)];
/// Each end of an inner wall moves this far along the outer wall.
pub const DIVIDER_SLANT: Fx = meters(6);
/// A bend in a long run moves this far off the straight line.
pub const DIVIDER_BEND: Fx = meters(3);
/// A run at least this long gets a bend.
pub const DIVIDER_BEND_RUN: Fx = meters(35);

// Gaps. A wall has one or two; the slots are where a gap's center sits, in
// z, before it moves. One-gap walls stagger between the two ONE_GAP slots.
pub const GAP_WIDTH_MIN: Fx = meters(6);
pub const GAP_WIDTH_MAX: Fx = meters(10);
pub const TWO_GAPS_CHANCE: (u32, u32) = (1, 2);
pub const ONE_GAP_SLOTS: [Fx; 2] = [meters(62), meters(113)];
pub const TWO_GAP_SLOTS: [Fx; 2] = [meters(47), meters(128)];
/// How far a seed moves a gap from its slot, along the wall.
pub const GAP_MOVE: Fx = meters(11);
/// And across the wall's line.
pub const GAP_OFFSET: Fx = meters(2);
/// The gap's tilt from the wall's line, as x gained per meter of z.
pub const GAP_TILT: Fx = tenths(1);

// The Fold's own gap, in its front wall.
pub const FOLD_GAP_WIDTH: Fx = meters(8);
/// How far a seed moves it along the front wall.
pub const FOLD_GAP_MOVE: Fx = meters(5);
/// Where a folded sheep heads for, this far in from the back wall.
pub const FOLD_BACK_INSET: Fx = meters(5);

// Where each field's random spots are drawn from, in x. Spots are tested
// against the real walls, so these only need to cover the field.
pub const FIELD_SPAN: [(Fx, Fx); 3] = [
    (meters(0), meters(125)),
    (meters(95), meters(230)),
    (meters(197), meters(300)),
];
/// Each field's rough area, in square meters.
pub const FIELD_AREA: [i32; 3] = [17600, 17500, 13100];

// Placing. A spot is tried this many times, and each round after the first
// eases the spacing between props by an eighth.
pub const PLACE_TRIES: usize = 200;
pub const PLACE_ROUNDS: i32 = 6;

// Clearances that keep the flock's route as wide as the gaps. A tree or
// cairn stands this far clear of every wall and from each gap's center.
pub const PROP_WALL_CLEAR: Fx = meters(7);
pub const PROP_GAP_CLEAR: Fx = meters(18);
/// And this far from either end of an inner wall. A Sheepdog behind a tree
/// nearer than this holds sheep in the wedge there, inside its flight radius.
pub const PROP_WALL_END_CLEAR: Fx = meters(18);

// Sparse, lone, large trees: how many in each field.
pub const TREE_QUOTA: [usize; 3] = [6, 5, 5];
pub const TREE_SPACING: Fx = meters(30);
pub const TRUNK_RADIUS: (Fx, Fx) = (tenths(6), tenths(10));
pub const CROWN_RADIUS: (Fx, Fx) = (meters(5), meters(8));
pub const TREE_HEIGHT: (Fx, Fx) = (meters(9), meters(14));

// Young trees in small groups, as solid as any other tree. Neighbors stand
// far enough apart for the flock's route to run between them.
pub const GROVE_QUOTA: [usize; 3] = [2, 2, 1];
pub const GROVE_TREES: (i32, i32) = (2, 3);
/// From one trunk's surface to the next.
pub const GROVE_CLEAR: Fx = (105 * ONE) / 10;
/// How far past that a neighbor may stand.
pub const GROVE_SPREAD: Fx = meters(4);
pub const GROVE_TRIES: usize = 40;
pub const GROVE_TRUNK: (Fx, Fx) = (tenths(3), tenths(4));
pub const GROVE_CROWN: (Fx, Fx) = (tenths(30), tenths(42));
pub const GROVE_HEIGHT: (Fx, Fx) = (meters(5), meters(8));

// Cairns: stepped, and climbed by jumping from step to step. Sheep can't.
pub const CAIRN_QUOTA: [usize; 3] = [3, 3, 3];
pub const CAIRN_SPACING: Fx = meters(45);
/// A cairn keeps this far from any tree.
pub const CAIRN_TREE_CLEAR: Fx = meters(16);
/// Of this many good spots, a cairn takes the one highest on its rise.
pub const CAIRN_CANDIDATES: usize = 5;
pub const CAIRN_STEPS: (i32, i32) = (3, 4);
pub const CAIRN_TOP_RADIUS: Fx = tenths(10);
/// The width of each step.
pub const CAIRN_TREAD: Fx = tenths(9);
/// The height of each step.
pub const CAIRN_RISE: Fx = (55 * ONE) / 100;
/// Feet this close below a step walk onto it.
pub const CAIRN_STEP_UP: Fx = tenths(2);

// Slabs: long low stones through the turf. Stone like a cairn's: everything
// walking stops at one, and a jump lands on top. Sheep can't.
/// One try at a slab in each square this wide.
pub const SLAB_CELL: Fx = meters(12);
/// Percent of squares that try. The clearances turn many away.
pub const SLAB_FILL: u32 = 30;
/// From the middle of the slab to the middle of its end.
pub const SLAB_REACH: (Fx, Fx) = (tenths(14), tenths(28));
/// Half its width.
pub const SLAB_HALF: (Fx, Fx) = (tenths(7), tenths(11));
/// Above the ground. All past the step up, so none can be walked onto.
pub const SLAB_TOP: (Fx, Fx) = ((38 * ONE) / 100, (58 * ONE) / 100);
/// Feet this close below a slab's top walk onto it.
pub const SLAB_STEP_UP: Fx = CAIRN_STEP_UP;
/// Clear of a tree, a cairn or another slab: a route for the flock, wider
/// than any gap.
pub const SLAB_PROP_CLEAR: Fx = meters(10);
/// Clear of a tunnel's mouth.
pub const SLAB_MOUTH_CLEAR: Fx = meters(4);

// Tunnels: fixed pairs of mouths. Each entry is the two fields one tunnel
// joins, so a tunnel always spans real distance.
pub const TUNNEL_FIELDS: [(usize, usize); 4] = [(0, 2), (0, 1), (1, 2), (0, 2)];
pub const TUNNEL_LENGTH_MIN: Fx = meters(112);
pub const MOUTH_SPACING: Fx = meters(50);
pub const MOUTH_WALL_CLEAR: Fx = meters(5);
pub const MOUTH_GAP_CLEAR: Fx = meters(16);
/// No mouth this close to the Fold's gap, so no Wolf comes up beside it.
pub const MOUTH_FOLD_CLEAR: Fx = meters(30);
/// From the surface of a trunk or cairn.
pub const MOUTH_PROP_CLEAR: Fx = meters(6);

// Mounds. Every spot sits at the base of a tree or cairn, and no tree or
// cairn has more than two.
pub const MOUNDS_AT_START: usize = 16;
/// Spare spots, in the order they come up: about one a minute over the
/// default 8-minute match.
pub const SPARE_MOUNDS: usize = 16;
/// From the surface of the trunk or cairn to the mound's center.
pub const MOUND_OFFSET: Fx = tenths(12);

// Start spots.
pub const SHEEPDOG_STARTS: usize = 8;
/// Sheepdogs start on an arc in front of the Fold's gap.
pub const SHEEPDOG_ARC_RADIUS: (Fx, Fx) = (meters(11), meters(14));
/// The arc's half angle, in turns: 3/20 of a turn is 54 degrees.
pub const SHEEPDOG_ARC_TURNS: i32 = crate::fx::TURN * 3 / 20;
/// Each field's share of the flock, in percent, weighted to the low end so
/// the gaps matter.
pub const SHEEP_SHARE: [usize; 3] = [45, 35, 20];
/// How many sheep a starting clump holds, and how many clumps in 100 hold
/// that many: a mean of 13 (the person: "make the initial clumps of sheep
/// larger").
pub const SHEEP_CLUMPS: [(usize, u32); 4] = [(8, 25), (12, 40), (16, 25), (24, 10)];
/// The kth sheep of a clump stands this far times root (k + 1) from its
/// first, so a clump is a disk of sheep close to shoulder to shoulder.
pub const SHEEP_CLUMP_PACK: Fx = tenths(65) / 10;
/// And no nearer than this to another of the clump.
pub const SHEEP_CLUMP_GAP: Fx = tenths(9);
/// And this much either way of that, so it doesn't read as a pattern.
pub const SHEEP_CLUMP_LOOSE: Fx = tenths(1);
/// A clump's first sheep starts this far clear of walls, trunks and cairns,
/// so the clump isn't cut in half by one.
pub const SHEEP_CLUMP_CLEAR: Fx = meters(4);
/// Spacing between groups is this share of the even spacing, in percent.
pub const SHEEP_SPACING_PERCENT: i32 = 70;
/// A sheep starts this far clear of walls, trunks and cairns.
pub const SHEEP_START_CLEAR: Fx = tenths(12);
pub const SHEEP_GAP_CLEAR: Fx = meters(7);
pub const SHEEP_FOLD_CLEAR: Fx = meters(20);
pub const SHEEP_MOUTH_CLEAR: Fx = tenths(25);
pub const SHEEP_MOUND_CLEAR: Fx = meters(2);

// Dressing: the low plants and pebbles that keep the hillside from looking
// bare. Picture only: no body ever meets it, and it never stands as tall as
// a crouched Wolf's shoulder (0.62 m), so it hides nobody.
pub const DRESS_TALLEST: Fx = tenths(6);
/// One spot in each square this wide.
pub const DRESS_CELL: Fx = meters(6);
/// Percent of squares that hold something: inside the walls, and past them.
pub const DRESS_FILL: [i32; 2] = [86, 34];
/// How many more heather or bracken stand round each one.
pub const DRESS_DRIFT: i32 = 2;
/// Each kind's width and height, each a range in centimeters: heather,
/// bracken, gorse, bush, rock, thistle, rushes.
pub const DRESS_SIZES: [((i32, i32), (i32, i32)); 7] = [
    ((70, 130), (24, 40)),
    ((90, 150), (34, 52)),
    ((80, 140), (42, 58)),
    ((70, 120), (36, 56)),
    ((16, 38), (8, 16)),
    ((30, 46), (34, 54)),
    ((40, 70), (32, 50)),
];
/// Clear of a wall, a gap's center, a tunnel's mouth, a mound, and the
/// surface of a trunk or cairn.
pub const DRESS_WALL_CLEAR: Fx = tenths(15);
pub const DRESS_GAP_CLEAR: Fx = meters(5);
pub const DRESS_MOUTH_CLEAR: Fx = meters(3);
pub const DRESS_MOUND_CLEAR: Fx = meters(2);
pub const DRESS_PROP_CLEAR: Fx = meters(1);
/// One boggy patch at most in each square this wide, this often in percent.
pub const BOG_CELL: Fx = meters(44);
pub const BOG_CHANCE: i32 = 40;
pub const BOG_RADIUS: (Fx, Fx) = (tenths(16), tenths(36));
pub const BOG_GAP_CLEAR: Fx = meters(12);
