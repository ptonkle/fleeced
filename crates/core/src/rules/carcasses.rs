//! Carcasses the Sheepdogs have found. A Sheepdog finds one by putting the
//! crosshair on it with a clear line of sight. `World` works that out and
//! passes it in as `sighted`. Each carcass counts once for the whole team.

/// A carcass, found or not yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Carcass {
    found: bool,
}

impl Carcass {
    pub fn found(&self) -> bool {
        self.found
    }
}

/// The Sheepdogs' count: their HUD's Carcasses found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CarcassTally {
    found: i32,
}

impl CarcassTally {
    pub fn carcasses_found(&self) -> i32 {
        self.found
    }

    /// One tick for one carcass. True on the tick it's first found.
    pub fn spot(&mut self, carcass: &mut Carcass, sighted: bool) -> bool {
        if !sighted || carcass.found {
            return false;
        }
        carcass.found = true;
        self.found += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_carcass_nobody_has_sighted_stays_unfound() {
        let (mut tally, mut carcass) = (CarcassTally::default(), Carcass::default());
        assert!(!tally.spot(&mut carcass, false));
        assert!(!carcass.found());
        assert_eq!(tally.carcasses_found(), 0);
    }

    #[test]
    fn a_carcass_is_found_once_however_long_it_is_watched() {
        let (mut tally, mut carcass) = (CarcassTally::default(), Carcass::default());
        assert!(tally.spot(&mut carcass, true));
        assert!(!tally.spot(&mut carcass, true));
        assert!(carcass.found());
        assert_eq!(tally.carcasses_found(), 1);
    }

    #[test]
    fn each_carcass_found_adds_one_to_the_count() {
        let mut tally = CarcassTally::default();
        let (mut first, mut second) = (Carcass::default(), Carcass::default());
        tally.spot(&mut first, true);
        tally.spot(&mut second, true);
        assert_eq!(tally.carcasses_found(), 2);
    }
}
