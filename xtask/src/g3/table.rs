//! The shape of each G3 position (EN 300 706 clause 15.7.2, Table 48).

use super::geometry::{
    CENTRE, EDGE_LEFT, EDGE_RIGHT, FULL_HEIGHT, FULL_WIDTH, HEAVY_BOTTOM, HEAVY_TOP, LIGHT_BOTTOM, LIGHT_LEFT,
    LIGHT_RIGHT, LIGHT_TOP, MIDDLE, Part, Shape, X, Y, complement, heavy_horizontal, light_horizontal, light_vertical,
    mirror, polygon, rect, stroke, union,
};

/// The shape of a G3 position (Table 48), or `None` for the positions
/// with no character and for 5/F, the background-colour space.
#[allow(clippy::too_many_lines, reason = "one arm per position of Table 48")]
pub(super) fn shape(code: u8) -> Option<Shape> {
    use X::{Centre as XC, Left as XL, Right as XR};
    use Y::{Bottom as B, Half as H, Third as T1, Top as T, TwoThirds as T2};
    let mirrored = |code: u8| shape(code).map(|shape| mirror(&shape));
    Some(match code {
        // Columns 2 and 3, and their complements in columns 6 and 7.
        0x20 => polygon(&[(XC, B), (XL, B), (XL, T2)]),
        0x21 => polygon(&[(XR, B), (XL, B), (XL, T2)]),
        0x22 => polygon(&[(XC, B), (XL, B), (XL, T1)]),
        0x23 => polygon(&[(XR, B), (XL, B), (XL, T1)]),
        0x24 => polygon(&[(XL, T), (XC, B), (XL, B)]),
        0x25 => polygon(&[(XL, T), (XR, B), (XL, B)]),
        0x26 => polygon(&[(XC, T), (XR, T), (XR, B), (XL, B), (XL, T1)]),
        0x27 => polygon(&[(XR, T), (XR, B), (XL, B), (XL, T1)]),
        0x28 => polygon(&[(XC, T), (XR, T), (XR, B), (XL, B), (XL, T2)]),
        0x29 => polygon(&[(XR, T), (XR, B), (XL, B), (XL, T2)]),
        0x2A => polygon(&[(XC, T), (XR, T), (XR, B), (XL, B)]),
        0x2B => polygon(&[(XR, T1), (XR, B), (XL, B), (XL, T2)]),
        0x2C => polygon(&[(XL, T), (XR, T), (XR, B), (XL, B), (XC, H)]),
        0x2D => polygon(&[(XL, T), (XC, H), (XR, T), (XR, B), (XL, B)]),
        // 2/E and 3/E: the eighth blocks 2 and 7 of L2/19-025, drawn 2
        // half-dots wide (decision E.50), as mirror images.
        0x2E => union(vec![rect(2, 6, 0, FULL_HEIGHT)]),
        0x3E => union(vec![rect(18, 22, 0, FULL_HEIGHT)]),
        0x2F => union(vec![Part::Checker]),
        0x30..=0x3B => return mirrored(code - 0x10),
        0x3C => polygon(&[(XL, T), (XR, T), (XC, H), (XR, B), (XL, B)]),
        0x3D => polygon(&[(XL, T), (XR, T), (XR, B), (XC, H), (XL, B)]),
        0x3F => polygon(&[(XL, T), (XR, T), (XR, B), (XL, B)]),
        0x60..=0x6B | 0x70..=0x7B => return shape(code - 0x40).map(|shape| complement(&shape)),
        0x6C => polygon(&[(XL, T), (XL, B), (XC, H)]),
        0x6D => polygon(&[(XR, T), (XL, T), (XC, H)]),
        0x7C => polygon(&[(XR, B), (XR, T), (XC, H)]),
        0x7D => polygon(&[(XL, B), (XR, B), (XC, H)]),
        // Column 4: heavy and light line pieces, chevrons, diagonals and
        // discs.
        0x40 => union(vec![heavy_horizontal(0, FULL_WIDTH), light_vertical(0, HEAVY_TOP)]),
        0x41 => union(vec![heavy_horizontal(0, FULL_WIDTH), light_vertical(HEAVY_BOTTOM, FULL_HEIGHT)]),
        0x42 => union(vec![light_vertical(0, FULL_HEIGHT), heavy_horizontal(LIGHT_LEFT, FULL_WIDTH)]),
        0x43 => union(vec![light_vertical(0, FULL_HEIGHT), heavy_horizontal(0, LIGHT_RIGHT)]),
        0x44 => {
            union(vec![stroke((CENTRE, 0), (EDGE_LEFT, MIDDLE)), stroke((EDGE_LEFT, MIDDLE), (CENTRE, FULL_HEIGHT))])
        }
        0x45 => return mirrored(0x44),
        0x46 => union(vec![
            stroke((EDGE_LEFT, MIDDLE), (CENTRE, FULL_HEIGHT)),
            stroke((CENTRE, FULL_HEIGHT), (EDGE_RIGHT, MIDDLE)),
        ]),
        0x47 => union(vec![stroke((EDGE_LEFT, MIDDLE), (CENTRE, 0)), stroke((CENTRE, 0), (EDGE_RIGHT, MIDDLE))]),
        0x48 => union(vec![stroke((CENTRE, 0), (EDGE_LEFT, MIDDLE))]),
        0x49 => return mirrored(0x48),
        0x4A => union(vec![stroke((EDGE_LEFT, MIDDLE), (CENTRE, FULL_HEIGHT))]),
        0x4B => return mirrored(0x4A),
        0x4C => union(vec![light_vertical(0, FULL_HEIGHT), heavy_horizontal(0, FULL_WIDTH)]),
        // 4/D a bullet, 4/E a disc and 4/F a ring, as measured from the
        // figure: about 6 half-dots by 4 rows, the full width by 8 rows, and
        // the disc's outline about 2 half-dots thick.
        0x4D => union(vec![Part::Ellipse { cx: CENTRE, cy: MIDDLE, rx: 6, ry: 4 }]),
        0x4E => union(vec![Part::Ellipse { cx: CENTRE, cy: MIDDLE, rx: 12, ry: 8 }]),
        0x4F => Shape::new(
            vec![Part::Ellipse { cx: CENTRE, cy: MIDDLE, rx: 12, ry: 8 }],
            vec![Part::Ellipse { cx: CENTRE, cy: MIDDLE, rx: 8, ry: 5 }],
        ),
        // Column 5: light box drawing, arms meeting at half-dots 5 and 6
        // and rows 4 and 5.
        0x50 => union(vec![light_vertical(0, FULL_HEIGHT)]),
        0x51 => union(vec![light_horizontal(0, FULL_WIDTH)]),
        0x52 => union(vec![light_horizontal(LIGHT_LEFT, FULL_WIDTH), light_vertical(LIGHT_TOP, FULL_HEIGHT)]),
        0x53 => union(vec![light_horizontal(0, LIGHT_RIGHT), light_vertical(LIGHT_TOP, FULL_HEIGHT)]),
        0x54 => union(vec![light_horizontal(LIGHT_LEFT, FULL_WIDTH), light_vertical(0, LIGHT_BOTTOM)]),
        0x55 => union(vec![light_horizontal(0, LIGHT_RIGHT), light_vertical(0, LIGHT_BOTTOM)]),
        0x56 => union(vec![light_vertical(0, FULL_HEIGHT), light_horizontal(LIGHT_LEFT, FULL_WIDTH)]),
        0x57 => union(vec![light_vertical(0, FULL_HEIGHT), light_horizontal(0, LIGHT_RIGHT)]),
        0x58 => union(vec![light_horizontal(0, FULL_WIDTH), light_vertical(LIGHT_TOP, FULL_HEIGHT)]),
        0x59 => union(vec![light_horizontal(0, FULL_WIDTH), light_vertical(0, LIGHT_BOTTOM)]),
        0x5A => union(vec![light_horizontal(0, FULL_WIDTH), light_vertical(0, FULL_HEIGHT)]),
        // Triangle-headed arrows: a light shaft and a head; 5/B right, 5/C
        // left, 5/D up and 5/E down.
        0x5B => union(vec![
            light_horizontal(0, CENTRE),
            Part::Polygon(vec![
                (X::Design(CENTRE), Y::Design(4)),
                (X::Design(FULL_WIDTH), Y::Design(MIDDLE)),
                (X::Design(CENTRE), Y::Design(16)),
            ]),
        ]),
        0x5C => return mirrored(0x5B),
        0x5D => union(vec![
            light_vertical(LIGHT_TOP, FULL_HEIGHT),
            Part::Polygon(vec![
                (X::Design(4), Y::Design(LIGHT_TOP)),
                (X::Design(CENTRE), Y::Design(0)),
                (X::Design(20), Y::Design(LIGHT_TOP)),
            ]),
        ]),
        0x5E => union(vec![
            light_vertical(0, LIGHT_BOTTOM),
            Part::Polygon(vec![
                (X::Design(4), Y::Design(LIGHT_BOTTOM)),
                (X::Design(20), Y::Design(LIGHT_BOTTOM)),
                (X::Design(CENTRE), Y::Design(FULL_HEIGHT)),
            ]),
        ]),
        // 5/F shows the background colour (note 1); 6/E, 6/F, 7/E and 7/F
        // have no character (note 2).
        _ => return None,
    })
}
