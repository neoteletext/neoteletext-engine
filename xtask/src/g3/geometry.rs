//! The geometry the G3 shapes are drawn with: positions on the sextant grid
//! and the design grid, polygons, ellipses and the checkerboard, the strokes
//! of the line-drawing characters, and the rasteriser (EN 300 706 clause
//! 15.7.2, decision E.50).

use super::CELL_WIDTH;

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
pub(super) enum X {
    Left,
    Centre,
    Right,
    Design(i64),
}

/// A vertical position on the sextant grid, or a design position in half
/// rows of the 10-row design grid (0 to 20).
#[derive(Clone, Copy)]
pub(super) enum Y {
    Top,
    Third,
    Half,
    TwoThirds,
    Bottom,
    Design(i64),
}

/// One part of a glyph; a glyph is the union of its parts.
#[derive(Clone)]
pub(super) enum Part {
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
pub(super) struct Shape {
    set: Vec<Part>,
    clear: Vec<Part>,
}

impl Shape {
    /// The parts in `set`, minus the parts in `clear`.
    pub(super) fn new(set: Vec<Part>, clear: Vec<Part>) -> Self {
        Self { set, clear }
    }
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
    pub(super) fn doubled(self, height: i64) -> i64 {
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
pub(super) fn rasterise(shape: &Shape, height: usize) -> Vec<u16> {
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

pub(super) fn polygon(points: &[(X, Y)]) -> Shape {
    Shape { set: vec![Part::Polygon(points.to_vec())], clear: Vec::new() }
}

/// A rectangle in design half units: half half-dots across, half rows down.
pub(super) fn rect(x0: i64, x1: i64, y0: i64, y1: i64) -> Part {
    Part::Polygon(vec![
        (X::Design(x0), Y::Design(y0)),
        (X::Design(x1), Y::Design(y0)),
        (X::Design(x1), Y::Design(y1)),
        (X::Design(x0), Y::Design(y1)),
    ])
}

pub(super) fn union(parts: Vec<Part>) -> Shape {
    Shape { set: parts, clear: Vec::new() }
}

// The strokes of the line-drawing characters, measured from the figure of
// Table 48 on the 12 by 10 matrix (`research/notes-level25-drcs.md` section
// 9.3), in design half units. Light verticals are half-dots 5 and 6, light
// horizontals rows 4 and 5, heavy horizontals rows 3 to 6.
pub(super) const FULL_WIDTH: i64 = 24;
pub(super) const FULL_HEIGHT: i64 = 20;
pub(super) const LIGHT_LEFT: i64 = 10;
pub(super) const LIGHT_RIGHT: i64 = 14;
pub(super) const LIGHT_TOP: i64 = 8;
pub(super) const LIGHT_BOTTOM: i64 = 12;
pub(super) const HEAVY_TOP: i64 = 6;
pub(super) const HEAVY_BOTTOM: i64 = 14;

pub(super) fn light_vertical(y0: i64, y1: i64) -> Part {
    rect(LIGHT_LEFT, LIGHT_RIGHT, y0, y1)
}

pub(super) fn light_horizontal(x0: i64, x1: i64) -> Part {
    rect(x0, x1, LIGHT_TOP, LIGHT_BOTTOM)
}

pub(super) fn heavy_horizontal(x0: i64, x1: i64) -> Part {
    rect(x0, x1, HEAVY_TOP, HEAVY_BOTTOM)
}

/// A diagonal stroke 2 half-dots wide, measured across: a horizontal bar
/// swept from one end to the other. Ends are in design half units.
pub(super) fn stroke((ax, ay): (i64, i64), (bx, by): (i64, i64)) -> Part {
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
pub(super) const EDGE_LEFT: i64 = 2;
pub(super) const EDGE_RIGHT: i64 = 22;
pub(super) const CENTRE: i64 = 12;
pub(super) const MIDDLE: i64 = 10;

/// The shape mirrored left to right.
pub(super) fn mirror(shape: &Shape) -> Shape {
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
pub(super) fn complement(shape: &Shape) -> Shape {
    let full = Part::Polygon(vec![(X::Left, Y::Top), (X::Right, Y::Top), (X::Right, Y::Bottom), (X::Left, Y::Bottom)]);
    Shape { set: vec![full], clear: shape.set.clone() }
}
