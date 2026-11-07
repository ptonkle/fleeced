//! Everything that decides whether a Wolf can hide in a flock, in one table.
//! A Wolf is a real wolf up close. It hides by color, by the low crouch and by
//! matching a sheep's height and pace, so it blends only far off and in a
//! crowd.
//!
//! Change a number here and rebuild, or try one without a rebuild from the
//! hide scene's address (`gallery.html?scene=hide&w.height=55`) or its tuning
//! panel (`T`). Sizes are centimeters, angles degrees, the rest percent.

macro_rules! table {
    ($($key:ident: $value:literal,)*) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct Hiding {
            $(pub $key: i32,)*
        }

        pub const HIDING: Hiding = Hiding {
            $($key: $value,)*
        };

        impl Hiding {
            /// Every row by name, in the table's order.
            pub fn rows(&self) -> Vec<(&'static str, i32)> {
                vec![$((stringify!($key), self.$key),)*]
            }

            /// Sets a row by name; an unknown name changes nothing.
            pub fn set(&mut self, key: &str, value: i32) {
                match key {
                    $(stringify!($key) => self.$key = value,)*
                    _ => {}
                }
            }
        }
    };
}

table! {
    // how the crouch changes the silhouette
    height: 62,    // the crouched shoulder, cm
    pitch: 2,      // the body tips nose down, degrees
    arch: 3,       // the chest bends down from the loin, degrees
    neck: 62,      // the neck lowers, degrees
    head: -42,     // the head turns back up on the neck, degrees
    tail: -50,     // the tail swings in against the rump, degrees
    ears_back: 28, // the ears lie back, degrees
    gather: 10,    // the feet come in under the body, cm
    // how a plain Wolf carries itself on its feet
    carry: 34,     // the neck lowers and the head stays level, degrees
    // the whole animal in every pose, percent; the contract sizes it at 100
    width: 100,
    length: 100,
    // the head, percent
    head_size: 100,
    muzzle: 82,       // how long
    muzzle_wide: 128, // how wide and tall
    ears: 115,
    // the fur: which of the breed's coats, then shares of it in percent
    coat: 0,
    back: 58,
    flank: 102,
    legs: 88,
    face: 98,
    fur: 3,      // how far a tuft's tip stands off the skin, cm
    grain: 18,   // how many tufts round the body
    sturdy: 125, // how thick the legs, percent
    // the disguise (the Sheep Suit powerup): the sheep's soft wool
    shag: 14,    // the size of a lump, cm
    depth: 8,    // how far the coat hangs below the body, over the legs, cm
    clumps: 18,  // how many lumps
    puff: 32,    // how wide the coat stands, half width in cm
    disguise_legs: 150,
    disguise_head: 90,
    disguise_muzzle: 85,
    disguise_muzzle_wide: 110,
    disguise_ears: 75,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_is_set_by_name_and_an_unknown_name_changes_nothing() {
        let mut h = HIDING;
        h.set("height", 50);
        assert_eq!(h.height, 50);
        let row = h.rows().iter().find(|r| r.0 == "height").copied();
        assert_eq!(row, Some(("height", 50)));
        let before = h;
        h.set("wings", 2);
        assert_eq!(h, before);
    }
}
