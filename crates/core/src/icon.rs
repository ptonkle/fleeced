//! The tab icon, made here as an SVG data URL: a sheep's face on a brass-edged
//! heather shield. The page sets it on its `<link rel="icon">`; the CSP's
//! `img-src data:` lets it through. A data URL means no icon file to request
//! and no second copy of the art. After Crabble's and Honey Hole's tab icons.

/// The icon as SVG text. Sized on a 32-unit grid so it stays a face at 16
/// pixels: a pale fleece, a dark muzzle, two dark ears, one eye each side.
pub fn svg() -> String {
    concat!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 32 32'>",
        "<rect x='1' y='1' width='30' height='30' rx='7' fill='#2b2438' stroke='#b08d4a' stroke-width='2'/>",
        "<g fill='#efe6d2'>",
        "<circle cx='10' cy='11' r='5'/><circle cx='16' cy='9' r='5.5'/><circle cx='22' cy='11' r='5'/>",
        "<circle cx='8' cy='17' r='4.5'/><circle cx='24' cy='17' r='4.5'/>",
        "<circle cx='10' cy='22' r='4'/><circle cx='22' cy='22' r='4'/><circle cx='16' cy='24' r='4'/>",
        "</g>",
        "<g fill='#17131f'>",
        "<ellipse cx='5.5' cy='14' rx='3.5' ry='1.8'/><ellipse cx='26.5' cy='14' rx='3.5' ry='1.8'/>",
        "<ellipse cx='16' cy='17' rx='6' ry='7'/>",
        "</g>",
        "<circle cx='13.2' cy='15' r='1.4' fill='#efe6d2'/><circle cx='18.8' cy='15' r='1.4' fill='#efe6d2'/>",
        "</svg>"
    )
    .to_string()
}

/// What goes inside each powerup's icon, by its id in the powerup table: on a
/// 32-unit grid, drawn dark and plain to read at 36 pixels on the HUD's brass
/// medallion. The Howl's is the three wolf moon the person asked for.
fn powerup_art(id: &str) -> &'static str {
    match id {
        // jaws open, teeth showing
        "super-appetite" => concat!(
            "<path d='M4 12 Q16 2 28 12 L26 16 H6 Z' fill='#2b2438'/>",
            "<path d='M6 20 H26 Q16 30 6 20 Z' fill='#2b2438'/>",
            "<path d='M8 16 L10 12 L12 16 M14 16 L16 12 L18 16 M20 16 L22 12 L24 16' fill='#efe6d2'/>",
            "<path d='M10 20 L12 24 L14 20 M18 20 L20 24 L22 20' fill='#efe6d2'/>"
        ),
        // a hooded cloak, its edge broken up
        "invisibility-cloak" => concat!(
            "<path d='M16 3 Q8 5 8 14 L5 29 H27 L24 14 Q24 5 16 3 Z' fill='none' stroke='#2b2438' ",
            "stroke-width='2.5' stroke-dasharray='4 2.5' stroke-linejoin='round'/>",
            "<circle cx='13' cy='13' r='1.6' fill='#2b2438'/><circle cx='19' cy='13' r='1.6' fill='#2b2438'/>"
        ),
        // a wolf's face and ears through a fleece
        "sheep-suit" => concat!(
            "<path d='M7 3 L13 11 H19 L25 3 L24 17 H8 Z' fill='#2b2438'/>",
            "<g fill='#efe6d2' stroke='#2b2438' stroke-width='1.2'>",
            "<circle cx='8' cy='15' r='4.5'/><circle cx='24' cy='15' r='4.5'/>",
            "<circle cx='7' cy='22' r='4.5'/><circle cx='25' cy='22' r='4.5'/>",
            "<circle cx='12' cy='26' r='4.5'/><circle cx='20' cy='26' r='4.5'/>",
            "<circle cx='16' cy='12' r='4'/></g>",
            "<ellipse cx='16' cy='20' rx='5.5' ry='6.5' fill='#2b2438'/>",
            "<circle cx='13.8' cy='18.5' r='1.2' fill='#d8b23c'/><circle cx='18.2' cy='18.5' r='1.2' fill='#d8b23c'/>"
        ),
        "bone" => concat!(
            "<g transform='rotate(-40 16 16)' fill='#efe6d2' stroke='#2b2438' stroke-width='1.6'>",
            "<path d='M8 13 H24 V19 H8 Z'/>",
            "<circle cx='6.5' cy='12' r='3.8'/><circle cx='6.5' cy='20' r='3.8'/>",
            "<circle cx='25.5' cy='12' r='3.8'/><circle cx='25.5' cy='20' r='3.8'/>",
            "<path d='M7 13.5 H25 V18.5 H7 Z' stroke='none'/></g>"
        ),
        // three wolves howling under a full moon
        "three-wolf-moon" => concat!(
            "<circle cx='16' cy='11' r='9.5' fill='#fbf3cf' stroke='#2b2438' stroke-width='1.2'/>",
            "<g fill='#2b2438'>",
            "<path d='M2 30 L5 19 L6 15 L7.5 19 L12 13 L10.5 20 L12 30 Z'/>",
            "<path d='M20 30 L21.5 20 L20 13 L24.5 19 L26 15 L27 19 L30 30 Z'/>",
            "<path d='M11 30 L13 19 L14 14 L16 18 L18 14 L19 19 L21 30 Z'/></g>"
        ),
        // a hoof and the lines of it bolting
        "stampede" => concat!(
            "<path d='M19 6 Q30 8 28 22 H21 L20 16 L18 22 H11 Q10 11 19 6 Z' fill='#2b2438'/>",
            "<path d='M2 9 H10 M1 15 H8 M2 21 H7' stroke='#2b2438' stroke-width='2.4' stroke-linecap='round'/>"
        ),
        "carrot-on-a-stick" => concat!(
            "<path d='M3 28 L26 4' stroke='#6b4a26' stroke-width='2.6' stroke-linecap='round'/>",
            "<path d='M26 4 V12' stroke='#2b2438' stroke-width='1.2'/>",
            "<path d='M22 12 H30 L26 29 Z' fill='#d9772b' stroke='#2b2438' stroke-width='1.2' stroke-linejoin='round'/>"
        ),
        // up from a spring
        "super-jump" => concat!(
            "<path d='M16 2 L26 13 H20 V19 H12 V13 H6 Z' fill='#2b2438'/>",
            "<path d='M8 22 L24 24 L8 26 L24 28 L8 30' fill='none' stroke='#2b2438' stroke-width='2' ",
            "stroke-linejoin='round'/>"
        ),
        // a pawprint at speed
        "zoomies" => concat!(
            "<g fill='#2b2438'><ellipse cx='20' cy='20' rx='6.5' ry='5.5'/>",
            "<circle cx='12' cy='14' r='2.6'/><circle cx='17' cy='9' r='2.6'/>",
            "<circle cx='23' cy='9' r='2.6'/><circle cx='28' cy='14' r='2.6'/></g>",
            "<path d='M2 12 H7 M1 19 H9 M3 26 H10' stroke='#2b2438' stroke-width='2.4' stroke-linecap='round'/>"
        ),
        "dognoculars" => concat!(
            "<g fill='#2b2438'><rect x='3' y='8' width='11' height='16' rx='3'/>",
            "<rect x='18' y='8' width='11' height='16' rx='3'/><rect x='13' y='12' width='6' height='5'/></g>",
            "<circle cx='8.5' cy='20' r='3.4' fill='#a9c8d8'/><circle cx='23.5' cy='20' r='3.4' fill='#a9c8d8'/>"
        ),
        // a nose under rising scent
        "sniff-snoff" => concat!(
            "<path d='M9 22 Q16 15 23 22 Q24 28 16 29 Q8 28 9 22 Z' fill='#2b2438'/>",
            "<g fill='none' stroke='#2b2438' stroke-width='2.2' stroke-linecap='round'>",
            "<path d='M9 14 Q6 11 9 8 Q12 5 9 2'/><path d='M16 12 Q13 9 16 6 Q19 3 16 1'/>",
            "<path d='M23 14 Q20 11 23 8 Q26 5 23 2'/></g>"
        ),
        "bell" => concat!(
            "<path d='M16 3 Q24 4 24 14 Q24 20 28 24 H4 Q8 20 8 14 Q8 4 16 3 Z' fill='#2b2438'/>",
            "<circle cx='16' cy='27' r='3' fill='#2b2438'/>"
        ),
        _ => "",
    }
}

/// A powerup's icon as SVG text.
pub fn powerup_svg(id: &str) -> String {
    format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 32 32'>{}</svg>",
        powerup_art(id)
    )
}

/// A powerup's icon for an `<img>`.
pub fn powerup_url(id: &str) -> String {
    format!("data:image/svg+xml,{}", encode(&powerup_svg(id)))
}

/// Percent-encodes everything but the characters a URL keeps as they are.
fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3 / 2);
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The tab icon as the `href` of a `<link rel="icon">`.
pub fn data_url() -> String {
    format!("data:image/svg+xml,{}", encode(&svg()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding_keeps_only_the_characters_a_url_keeps() {
        // worked by hand: '<' is 3C, ' ' is 20, '#' is 23, '>' is 3E
        assert_eq!(encode("<a b#>"), "%3Ca%20b%23%3E");
        assert_eq!(encode("x-y_z.1~"), "x-y_z.1~");
    }

    #[test]
    fn every_powerup_has_an_icon_of_its_own() {
        use crate::rules::tune::POWERUPS;
        for (i, a) in POWERUPS.iter().enumerate() {
            assert!(!powerup_art(a.icon).is_empty(), "{}", a.icon);
            for b in &POWERUPS[i + 1..] {
                assert_ne!(powerup_art(a.icon), powerup_art(b.icon), "{}", a.icon);
            }
        }
    }

    #[test]
    fn the_tab_icon_is_an_svg_data_url_a_browser_can_read() {
        let url = data_url();
        let body = url.strip_prefix("data:image/svg+xml,").expect("an SVG data URL");
        assert!(body.starts_with("%3Csvg"), "{body}");
        for raw in ['<', '>', '#', '"', '\'', ' '] {
            assert!(!body.contains(raw), "{raw:?} would end or break the URL");
        }
    }
}
