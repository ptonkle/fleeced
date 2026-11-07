//! Map size: Small, Medium or Big, a match setting. Big is the table's own
//! map, and a smaller one is the same hillside drawn smaller: the walls,
//! gaps and fields move in with the outline. The Fold stays the same size,
//! because it is sized for the flock, and so do gap widths and every clearance
//! kept for a sheep's sake. The size travels with the seed in the settings.

use serde::{Deserialize, Serialize};

use crate::fx::Fx;

use super::table::{self, SizeRow};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapSize {
    Small,
    Medium,
    #[default]
    Big,
}

/// `v * top / bottom`, through i64.
fn scaled(v: Fx, top: Fx, bottom: Fx) -> Fx {
    (v as i64 * top as i64 / bottom as i64) as Fx
}

impl MapSize {
    pub const ALL: [MapSize; 3] = [MapSize::Small, MapSize::Medium, MapSize::Big];

    /// The size's key: the action value, and `map_size.<key>` in the strings.
    pub fn key(self) -> &'static str {
        match self {
            MapSize::Small => "small",
            MapSize::Medium => "medium",
            MapSize::Big => "big",
        }
    }

    pub fn parse(text: &str) -> Option<MapSize> {
        MapSize::ALL.into_iter().find(|size| size.key() == text)
    }

    /// This size's row of the table.
    pub fn row(self) -> &'static SizeRow {
        &table::SIZES[self as usize]
    }

    pub fn length(self) -> Fx {
        self.row().length
    }

    pub fn width(self) -> Fx {
        self.row().width
    }

    /// How far up the hill the Fold's front wall stands, near enough.
    fn front(self) -> Fx {
        self.length() - table::FOLD_DEPTH
    }

    /// A distance up the hill on Big, at this size.
    pub fn up(self, d: Fx) -> Fx {
        scaled(d, self.front(), MapSize::Big.front())
    }

    /// A distance across the hill on Big, at this size.
    pub fn over(self, d: Fx) -> Fx {
        scaled(d, self.width(), table::WIDTH)
    }

    /// A z on Big, at this size: drawn in toward the middle of the hill.
    pub fn across(self, z: Fx) -> Fx {
        self.width() / 2 + self.over(z - table::WIDTH / 2)
    }

    /// A corner of the Fold on Big, at this size: moved with the hill's top
    /// end and its middle, and no nearer to the Fold's other corners.
    pub fn fold_corner(self, x: Fx, z: Fx) -> (Fx, Fx) {
        (
            x - (table::LENGTH - self.length()),
            self.width() / 2 + (z - table::WIDTH / 2),
        )
    }

    /// An x on this size's hill, as the same distance from the top end on Big.
    pub fn from_top(self, x: Fx) -> Fx {
        x + table::LENGTH - self.length()
    }

    /// An area on Big, at this size, in square meters.
    pub fn area(self, area: i32) -> i32 {
        let up = area as i64 * self.front() as i64 / MapSize::Big.front() as i64;
        (up * self.width() as i64 / table::WIDTH as i64) as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::from_int;
    use crate::map::table::meters;

    #[test]
    fn big_is_todays_map_and_it_is_the_default() {
        assert_eq!(MapSize::default(), MapSize::Big);
        assert_eq!(
            (MapSize::Big.length(), MapSize::Big.width()),
            (meters(325), meters(175))
        );
        // Big changes no number it is handed, to the last bit
        for v in [0, 1, 12345, meters(137) + 3] {
            assert_eq!(MapSize::Big.up(v), v);
            assert_eq!(MapSize::Big.across(v), v);
            assert_eq!(MapSize::Big.fold_corner(v, v), (v, v));
        }
        assert_eq!(MapSize::Big.area(17600), 17600);
    }

    #[test]
    fn a_smaller_map_draws_in_toward_its_middle_and_keeps_the_fold_whole() {
        // Small is 195 by 105: its Fold's front stands at 169, 299 on Big
        let s = MapSize::Small;
        assert_eq!(s.up(from_int(299)), from_int(169));
        // 87.5 is Big's middle, and it comes to 52.5 at this width
        assert_eq!(s.across(table::WIDTH / 2), from_int(105) / 2);
        assert_eq!(s.across(0), 0);
        assert_eq!(s.across(table::WIDTH), from_int(105));
        // the Fold's back corner keeps its place at the top end, and one 25 m
        // from the middle stays 25 m from the middle
        let (x, z) = s.fold_corner(from_int(325), table::WIDTH / 2 + from_int(25));
        assert_eq!((x, z), (from_int(195), from_int(155) / 2));
    }

    #[test]
    fn each_size_has_its_key_and_reads_it_back() {
        for (size, key) in [
            (MapSize::Small, "small"),
            (MapSize::Medium, "medium"),
            (MapSize::Big, "big"),
        ] {
            assert_eq!(size.key(), key);
            assert_eq!(MapSize::parse(key), Some(size));
        }
        assert_eq!(MapSize::parse("huge"), None);
    }
}
