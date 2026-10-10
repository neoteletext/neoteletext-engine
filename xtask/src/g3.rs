//! The G3 smooth mosaics and line drawing set (EN 300 706 clause 15.7.2,
//! Table 48), drawn from the project's own geometry (decision E.50,
//! REQ-RND-037).
//!
//! - Positions in columns 2, 3, 6 and 7 are the polygons that Unicode
//!   document L2/19-025 gives the matching characters, on the "sextant"
//!   grid: x at 0, ½ or 1 of the width, y at 0, ⅓, ½, ⅔ or 1 of the height
//!   (`research/notes-level25-drcs.md` section 9.3). Points at ⅓ and ⅔ are
//!   rounded down to whole lines.
//! - Line drawing in columns 4 and 5 uses light strokes 2 half-dots wide
//!   (or 2 rows high) and heavy strokes 4 rows high, at the positions
//!   measured from the standard's figure; box-drawing arms reach the cell
//!   edges so neighbouring cells join.
//! - 2/F is a single-dot checkerboard on the 12-dot grid (decision E.50).
//!
//! A pixel is set when its centre lies inside the shape. The centre is
//! nudged an infinitesimal distance upwards, so no centre ever lies on an
//! edge: a polygon and its complement then share no pixel, and mirroring
//! left to right is exact. All arithmetic is on integers, so the output is
//! the same on every machine (REQ-RND-015).
//!
//! The output is a text file, one glyph per line, which the core embeds as
//! replaceable data (REQ-RND-020).

use std::fmt::Write as _;

/// Half-dots across a cell (REQ-RND-002).
pub const CELL_WIDTH: usize = 12;
/// The two cell heights in lines (REQ-RND-002, REQ-RND-003).
pub const HEIGHTS: [usize; 2] = [10, 20];
/// G3 codes 2/0 to 7/F.
pub const FIRST_CODE: u8 = 0x20;
pub const CODES: usize = 96;
/// Rows of the 12 by 10 design grid the line drawing is measured on.
const DESIGN_ROWS: i64 = 10;

/// A cell coordinate as a signed number for the geometry; every value here
/// is below 100.
fn signed(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// A horizontal position on the sextant grid, or a design position in
/// half half-dots (0 to 24).
#[derive(Clone, Copy)]
enum X {
    Left,
    Centre,
    Right,
    Design(i64),
}

/// A vertical position on the sextant grid, or a design position in half
/// rows of the 10-row design grid (0 to 20).
#[derive(Clone, Copy)]
enum Y {
    Top,
    Third,
    Half,
    TwoThirds,
    Bottom,
    Design(i64),
}

/// One part of a glyph; a glyph is the union of its parts.
#[derive(Clone)]
enum Part {
    /// A simple polygon, vertices in order.
    Polygon(Vec<(X, Y)>),
    /// An axis-aligned ellipse: centre and radii in design half units.
    Ellipse { cx: i64, cy: i64, rx: i64, ry: i64 },
    /// The checkerboard of 2/F.
    Checker,
}

/// A glyph: parts that are set, minus parts that are cleared (for the ring
/// of 4/F).
#[derive(Clone, Default)]
struct Shape {
    set: Vec<Part>,
    clear: Vec<Part>,
}

impl X {
    /// Doubled half-dots: pixel centres are odd numbers.
    fn doubled(self) -> i64 {
        match self {
            Self::Left => 0,
            Self::Centre => signed(CELL_WIDTH),
            Self::Right => 2 * signed(CELL_WIDTH),
            Self::Design(value) => value,
        }
    }
}

impl Y {
    /// Doubled lines of a cell `height` lines high: line centres are odd
    /// numbers. One third and two thirds are rounded down to whole lines
    /// (decision E.50).
    fn doubled(self, height: i64) -> i64 {
        match self {
            Self::Top => 0,
            Self::Third => 2 * (height / 3),
            Self::Half => height,
            Self::TwoThirds => 2 * (2 * height / 3),
            Self::Bottom => 2 * height,
            Self::Design(value) => value * height / DESIGN_ROWS,
        }
    }
}

/// True when the point (x, y − ε) is inside the polygon: count the edges
/// above it on a vertical ray. `x` is odd and every vertex x is even, so
/// no vertex lies on the ray.
fn inside_polygon(vertices: &[(i64, i64)], x: i64, y: i64) -> bool {
    let mut inside = false;
    for (index, &(ax, ay)) in vertices.iter().enumerate() {
        let (bx, by) = vertices[(index + 1) % vertices.len()];
        if (ax > x) == (bx > x) {
            continue;
        }
        // The edge's height at x is below y − ε exactly when it is below y:
        // ay + (x − ax)(by − ay)/(bx − ax) < y.
        let run = bx - ax;
        let left = ay * run + (x - ax) * (by - ay);
        let right = y * run;
        let above = if run > 0 { left < right } else { left > right };
        if above {
            inside = !inside;
        }
    }
    inside
}

fn covers(part: &Part, x: usize, y: usize, height: usize) -> bool {
    let (px, py) = (2 * signed(x) + 1, 2 * signed(y) + 1);
    let lines = signed(height);
    match part {
        Part::Polygon(vertices) => {
            let points: Vec<(i64, i64)> = vertices.iter().map(|(vx, vy)| (vx.doubled(), vy.doubled(lines))).collect();
            inside_polygon(&points, px, py)
        }
        Part::Ellipse { cx, cy, rx, ry } => {
            // Scale the design's vertical half units to doubled lines.
            let (cy, ry) = (cy * lines / DESIGN_ROWS, ry * lines / DESIGN_ROWS);
            let (dx, dy) = (px - cx, py - cy);
            dx * dx * ry * ry + dy * dy * rx * rx < rx * rx * ry * ry
        }
        Part::Checker => {
            // One dot per row of the 12 by 10 matrix, so rows double in the
            // 12 by 20 cell.
            let row = y * DESIGN_ROWS as usize / height;
            (x + row).is_multiple_of(2)
        }
    }
}

/// The lines of a shape, bit 11 the leftmost half-dot.
fn rasterise(shape: &Shape, height: usize) -> Vec<u16> {
    (0..height)
        .map(|y| {
            (0..CELL_WIDTH).fold(0u16, |mask, x| {
                let on = shape.set.iter().any(|part| covers(part, x, y, height))
                    && !shape.clear.iter().any(|part| covers(part, x, y, height));
                if on { mask | 1 << (CELL_WIDTH - 1 - x) } else { mask }
            })
        })
        .collect()
}

fn polygon(points: &[(X, Y)]) -> Shape {
    Shape { set: vec![Part::Polygon(points.to_vec())], clear: Vec::new() }
}

/// A rectangle in design half units: half half-dots across, half rows down.
fn rect(x0: i64, x1: i64, y0: i64, y1: i64) -> Part {
    Part::Polygon(vec![
        (X::Design(x0), Y::Design(y0)),
        (X::Design(x1), Y::Design(y0)),
        (X::Design(x1), Y::Design(y1)),
        (X::Design(x0), Y::Design(y1)),
    ])
}

fn union(parts: Vec<Part>) -> Shape {
    Shape { set: parts, clear: Vec::new() }
}

// The strokes of the line-drawing characters, measured from the figure of
// Table 48 on the 12 by 10 matrix (`research/notes-level25-drcs.md` section
// 9.3), in design half units. Light verticals are half-dots 5 and 6, light
// horizontals rows 4 and 5, heavy horizontals rows 3 to 6.
const FULL_WIDTH: i64 = 24;
const FULL_HEIGHT: i64 = 20;
const LIGHT_LEFT: i64 = 10;
const LIGHT_RIGHT: i64 = 14;
const LIGHT_TOP: i64 = 8;
const LIGHT_BOTTOM: i64 = 12;
const HEAVY_TOP: i64 = 6;
const HEAVY_BOTTOM: i64 = 14;

fn light_vertical(y0: i64, y1: i64) -> Part {
    rect(LIGHT_LEFT, LIGHT_RIGHT, y0, y1)
}

fn light_horizontal(x0: i64, x1: i64) -> Part {
    rect(x0, x1, LIGHT_TOP, LIGHT_BOTTOM)
}

fn heavy_horizontal(x0: i64, x1: i64) -> Part {
    rect(x0, x1, HEAVY_TOP, HEAVY_BOTTOM)
}

/// A diagonal stroke 2 half-dots wide, measured across: a horizontal bar
/// swept from one end to the other. Ends are in design half units.
fn stroke((ax, ay): (i64, i64), (bx, by): (i64, i64)) -> Part {
    const HALF_WIDTH: i64 = 2;
    Part::Polygon(vec![
        (X::Design(ax - HALF_WIDTH), Y::Design(ay)),
        (X::Design(ax + HALF_WIDTH), Y::Design(ay)),
        (X::Design(bx + HALF_WIDTH), Y::Design(by)),
        (X::Design(bx - HALF_WIDTH), Y::Design(by)),
    ])
}

/// The centre lines of the diagonal strokes keep the stroke inside the
/// cell: an end at the left or right edge is one half-dot in.
const EDGE_LEFT: i64 = 2;
const EDGE_RIGHT: i64 = 22;
const CENTRE: i64 = 12;
const MIDDLE: i64 = 10;

/// The shape of a G3 position (Table 48), or `None` for the positions
/// with no character and for 5/F, the background-colour space.
#[allow(clippy::too_many_lines)] // One arm per position of Table 48.
fn shape(code: u8) -> Option<Shape> {
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
        0x4F => Shape {
            set: vec![Part::Ellipse { cx: CENTRE, cy: MIDDLE, rx: 12, ry: 8 }],
            clear: vec![Part::Ellipse { cx: CENTRE, cy: MIDDLE, rx: 8, ry: 5 }],
        },
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

/// The shape mirrored left to right.
fn mirror(shape: &Shape) -> Shape {
    let flip = |part: &Part| match part {
        Part::Polygon(vertices) => Part::Polygon(
            vertices
                .iter()
                .rev()
                .map(|&(x, y)| {
                    let mirrored = match x {
                        X::Left => X::Right,
                        X::Right => X::Left,
                        X::Centre => X::Centre,
                        X::Design(value) => X::Design(FULL_WIDTH - value),
                    };
                    (mirrored, y)
                })
                .collect(),
        ),
        Part::Ellipse { cx, cy, rx, ry } => Part::Ellipse { cx: FULL_WIDTH - cx, cy: *cy, rx: *rx, ry: *ry },
        Part::Checker => Part::Checker,
    };
    Shape { set: shape.set.iter().map(flip).collect(), clear: shape.clear.iter().map(flip).collect() }
}

/// The whole cell minus the shape.
fn complement(shape: &Shape) -> Shape {
    let full = Part::Polygon(vec![(X::Left, Y::Top), (X::Right, Y::Top), (X::Right, Y::Bottom), (X::Left, Y::Bottom)]);
    Shape { set: vec![full], clear: shape.set.clone() }
}

/// The text form of the generated set: comment lines, then one line per
/// position and cell height, `c/r height` followed by one 12-bit line per
/// cell line in hexadecimal, top first, bit 11 the leftmost half-dot.
pub fn text() -> String {
    let mut out = String::new();
    out.push_str("# G3 smooth mosaics and line drawing set (EN 300 706 clause 15.7.2, Table 48).\n");
    out.push_str("# Generated by `cargo xtask glyphs` from the project's own geometry (decision E.50,\n");
    out.push_str("# REQ-RND-037): the polygons of Unicode document L2/19-025 for columns 2, 3, 6 and 7,\n");
    out.push_str("# and strokes measured from Table 48 for columns 4 and 5. No glyph is copied from a\n");
    out.push_str("# character-generator ROM or a font. Licence: as the rest of this repository.\n");
    out.push_str("# Format: position, cell height, then one 12-bit line per cell line, top first,\n");
    out.push_str("# bit 11 the leftmost half-dot. Do not edit by hand.\n");
    for offset in 0..CODES {
        let code = FIRST_CODE + offset as u8;
        let shape = shape(code).unwrap_or_default();
        for height in HEIGHTS {
            let _ = write!(out, "{}/{:X} {height}", code >> 4, code & 0x0F);
            for line in rasterise(&shape, height) {
                let _ = write!(out, " {line:03X}");
            }
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(code: u8, height: usize) -> Vec<u16> {
        rasterise(&shape(code).unwrap_or_default(), height)
    }

    fn mirror_lines(lines: &[u16]) -> Vec<u16> {
        lines.iter().map(|line| line.reverse_bits() >> (16 - CELL_WIDTH)).collect()
    }

    const FULL: u16 = (1 << CELL_WIDTH) - 1;

    /// REQ-RND-037: the symmetry checks of `research/notes-level25-drcs.md`
    /// section 9.3, exact at both cell heights.
    #[test]
    fn columns_mirror_and_complement() {
        for height in HEIGHTS {
            for row in 0..=0x0B {
                let column2 = lines(0x20 + row, height);
                assert_eq!(lines(0x30 + row, height), mirror_lines(&column2), "3/{row:X} mirrors 2/{row:X}");
                let complement2: Vec<u16> = column2.iter().map(|line| !line & FULL).collect();
                assert_eq!(lines(0x60 + row, height), complement2, "6/{row:X} complements 2/{row:X}");
                let complement3: Vec<u16> = lines(0x30 + row, height).iter().map(|line| !line & FULL).collect();
                assert_eq!(lines(0x70 + row, height), complement3, "7/{row:X} complements 3/{row:X}");
            }
            assert_eq!(lines(0x3E, height), mirror_lines(&lines(0x2E, height)));
            assert_eq!(lines(0x6C, height), mirror_lines(&lines(0x7C, height)));
        }
    }

    /// Decision E.50: ⅓ and ⅔ round down to whole lines (3 and 6 of 10, 6
    /// and 13 of 20). The vertex of 2/0 at ⅔ is then on line 6 of 10, whose
    /// leftmost pixel centre lies inside, and on line 13 of 20, whose
    /// leftmost pixel centre lies just outside the slope, so the fill
    /// starts on line 14.
    #[test]
    fn thirds_round_down() {
        assert_eq!((Y::Third.doubled(10), Y::TwoThirds.doubled(10)), (2 * 3, 2 * 6));
        assert_eq!((Y::Third.doubled(20), Y::TwoThirds.doubled(20)), (2 * 6, 2 * 13));
        let small = lines(0x20, 10);
        assert_eq!(small.iter().position(|&line| line != 0), Some(6));
        let large = lines(0x20, 20);
        assert_eq!(large.iter().position(|&line| line != 0), Some(14));
        assert_eq!(lines(0x3F, 10), vec![FULL; 10]);
    }

    /// Decision E.50: 2/F is a single-dot checkerboard, rows doubled in the
    /// 12 by 20 cell; the empty positions are empty.
    #[test]
    fn checkerboard_and_empty_positions() {
        assert_eq!(&lines(0x2F, 10)[..2], &[0b1010_1010_1010, 0b0101_0101_0101]);
        assert_eq!(&lines(0x2F, 20)[..4], &[0b1010_1010_1010, 0b1010_1010_1010, 0b0101_0101_0101, 0b0101_0101_0101]);
        for code in [0x5F, 0x6E, 0x6F, 0x7E, 0x7F] {
            assert!(lines(code, 20).iter().all(|&line| line == 0));
        }
    }

    /// Light strokes are 2 half-dots wide and heavy strokes 4 rows high
    /// (decision E.50); box-drawing arms reach the cell edges.
    #[test]
    fn stroke_widths() {
        assert_eq!(lines(0x50, 10), vec![0b0000_0110_0000; 10]);
        let horizontal = lines(0x51, 10);
        assert_eq!(horizontal.iter().filter(|&&line| line == FULL).count(), 2);
        assert_eq!((horizontal[4], horizontal[5]), (FULL, FULL));
        let heavy = lines(0x4C, 20);
        assert_eq!(heavy.iter().filter(|&&line| line == FULL).count(), 8);
        let corner = lines(0x52, 10);
        assert_eq!(corner[9], 0b0000_0110_0000, "the down arm reaches the bottom edge");
        assert_eq!(corner[4] & 1, 1, "the right arm reaches the right edge");
    }
}
