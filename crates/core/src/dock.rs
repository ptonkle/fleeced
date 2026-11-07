//! Where the lobby's panels sit around the lineup, the turning characters of
//! every seat on the one full-window canvas (Ape Race's T47 and
//! `app/menu_dock.rs`). On a wide window the panels dock left and right and
//! the middle stays open; on a narrow one they go below and the lineup keeps
//! the top. The page only applies the dock it is given (`body[data-dock]`,
//! `--dock-w`, `--dock-top` in `web/style.css`), so the lineup's framing,
//! which reads the same `Dock`, and the layout cannot disagree.

use serde::{Deserialize, Serialize};

/// A docked panel's width in CSS pixels.
pub const PANEL_W: u32 = 400;
/// The space around and between docked panels, in CSS pixels.
pub const GAP: u32 = 24;
/// The narrowest open middle that still shows the lineup.
pub const OPEN_MIN: u32 = 360;
/// With the panels below, the share of the height the lineup keeps, percent.
pub const OPEN_TOP_PERCENT: u32 = 46;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "at", rename_all = "snake_case")]
pub enum Dock {
    /// Not a lobby: the panels sit in the middle.
    Center,
    /// Panels left and right, `open_w` wide open from `open_x` for the lineup.
    Sides { panel: u32, open_x: u32, open_w: u32 },
    /// Panels below `top`, and the lineup above.
    Below { top: u32 },
}

/// The dock for a lobby in a window this size, in CSS pixels.
pub fn lobby(width: u32, height: u32) -> Dock {
    let sides = 2 * (PANEL_W + 2 * GAP);
    if width >= sides + OPEN_MIN {
        Dock::Sides {
            panel: PANEL_W,
            open_x: PANEL_W + 2 * GAP,
            open_w: width - sides,
        }
    } else {
        Dock::Below {
            top: height * OPEN_TOP_PERCENT / 100,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wide_window_docks_the_panels_left_and_right_and_opens_the_middle() {
        assert_eq!(
            lobby(1920, 1080),
            Dock::Sides {
                panel: 400,
                open_x: 448,
                open_w: 1024
            }
        );
    }

    #[test]
    fn a_window_too_narrow_for_the_lineup_puts_the_panels_below() {
        assert_eq!(lobby(1255, 800), Dock::Below { top: 368 });
        assert!(matches!(lobby(1256, 800), Dock::Sides { open_w: 360, .. }));
    }
}
