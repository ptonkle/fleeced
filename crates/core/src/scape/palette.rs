//! The hillside's colors, measured off the reference photos
//! (`notes/hillside-look.md` gives each photo's numbers). Each channel runs
//! 0 to `ONE`.

use crate::fx::ONE;

pub type Tone = [i32; 3];

/// A tone from 0-255 channels.
pub const fn tone(r: i32, g: i32, b: i32) -> Tone {
    [r * ONE / 255, g * ONE / 255, b * ONE / 255]
}

/// `a` to `b`, `t` of the way.
pub fn mix(a: Tone, b: Tone, t: i32) -> Tone {
    [
        super::noise::lerp(a[0], b[0], t),
        super::noise::lerp(a[1], b[1], t),
        super::noise::lerp(a[2], b[2], t),
    ]
}

/// A tone scaled by percent.
pub fn pct(c: Tone, p: i32) -> Tone {
    c.map(|v| (v as i64 * p as i64 / 100) as i32)
}

// Inside the walls: the grazed turf.
pub const TURF: Tone = tone(112, 134, 62);
pub const TURF_DEEP: Tone = tone(92, 114, 50);
pub const TURF_LUSH: Tone = tone(124, 146, 72);
/// Where the sward runs thin and pale.
pub const TURF_BLEACHED: Tone = tone(146, 142, 88);
/// The strip sheep can't reach along a wall's foot.
pub const WALL_FOOT: Tone = tone(86, 100, 50);
/// Trampled ground: a gap, a tree's foot, the Fold.
pub const WORN: Tone = tone(116, 100, 66);
pub const WORN_DARK: Tone = tone(76, 64, 44);
/// Rush clumps: dark at the foot, straw at the tip.
pub const RUSH_FOOT: Tone = tone(66, 88, 34);
pub const RUSH_TIP: Tone = tone(148, 140, 70);

// Outside the walls: the rough hill the dyke keeps out.
pub const MOOR: Tone = tone(128, 122, 66);
pub const MOOR_PALE: Tone = tone(152, 140, 90);
pub const BRACKEN: Tone = tone(122, 92, 54);
pub const HEATHER: Tone = tone(102, 76, 80);

// The hillside's low plants, and wet ground.
pub const HEATHER_BLOOM: Tone = tone(120, 82, 112);
pub const HEATHER_DARK: Tone = tone(92, 96, 62);
pub const BRACKEN_GREEN: Tone = tone(104, 122, 52);
pub const BRACKEN_TURNED: Tone = tone(146, 110, 56);
pub const GORSE: Tone = tone(56, 76, 40);
pub const GORSE_FLOWER: Tone = tone(214, 176, 44);
pub const THISTLE_LEAF: Tone = tone(84, 104, 72);
pub const THISTLE_HEAD: Tone = tone(146, 84, 150);
pub const PEAT: Tone = tone(38, 40, 36);
pub const BOG_WATER: Tone = tone(66, 78, 82);
pub const BOG_MOSS: Tone = tone(136, 142, 58);

// The hills past the walls, before distance gets to them. Each is a photo's
// color over the 1.3 a slope in full sun adds.
pub const HILL_OLIVE: Tone = tone(110, 110, 66);
pub const HILL_TAWNY: Tone = tone(126, 106, 66);
pub const HILL_HEATHER: Tone = tone(96, 76, 74);
/// What pools in a valley between two ridges.
pub const HILL_MIST: Tone = tone(144, 158, 170);

// Stone.
pub const WHIN: Tone = tone(107, 109, 101);
pub const WHIN_DARK: Tone = tone(82, 84, 84);
pub const WHIN_WARM: Tone = tone(122, 114, 96);
pub const WHIN_TAN: Tone = tone(134, 118, 84);
pub const WHIN_PALE: Tone = tone(150, 152, 142);
pub const WHIN_MOSSY: Tone = tone(96, 110, 60);
/// Lichen flecks on a stone's upper faces.
pub const LICHEN: Tone = tone(206, 206, 188);
pub const LICHEN_GOLD: Tone = tone(190, 172, 92);
/// What shows between the stones.
pub const JOINT: Tone = tone(34, 34, 32);
/// Cairns and outcrops: paler and pinker than dyke stone.
pub const GRANITE: Tone = tone(148, 138, 132);
pub const GRANITE_DARK: Tone = tone(116, 115, 112);

// Trees: much darker than the turf, so one reads from anywhere.
pub const LEAF_LIT: Tone = tone(82, 104, 50);
pub const LEAF: Tone = tone(62, 80, 38);
pub const LEAF_SHADE: Tone = tone(44, 58, 26);
pub const ROWAN_LIT: Tone = tone(80, 98, 46);
pub const ROWAN: Tone = tone(58, 72, 36);
pub const PINE_LIT: Tone = tone(78, 104, 64);
pub const PINE: Tone = tone(50, 74, 56);
pub const PINE_SHADE: Tone = tone(32, 50, 42);
pub const BARK: Tone = tone(70, 60, 50);
pub const BARK_PALE: Tone = tone(120, 110, 96);
pub const PINE_BARK: Tone = tone(136, 96, 66);

// Dug earth.
pub const SOIL: Tone = tone(84, 62, 42);
pub const SOIL_DARK: Tone = tone(54, 40, 30);
pub const SOIL_PALE: Tone = tone(120, 94, 64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turf_is_green_enough_to_read_as_grass() {
        // the hill pasture photos' green beats red by 17 to 29 of 255 and
        // blue by 54 to 70 (`fold-meggethead`, `turf-eastertoun`)
        for c in [TURF, TURF_DEEP, TURF_LUSH] {
            let (over_red, over_blue) = (20 * ONE / 255, 54 * ONE / 255);
            assert!(c[1] - c[0] >= over_red && c[1] - c[2] >= over_blue, "{c:?}");
        }
    }

    #[test]
    fn a_tree_is_darker_than_the_turf_it_stands_on() {
        let sum = |c: Tone| c[0] + c[1] + c[2];
        for leaf in [LEAF_LIT, ROWAN_LIT, PINE_LIT] {
            assert!(sum(leaf) < sum(TURF_DEEP), "{leaf:?}");
        }
    }
}
