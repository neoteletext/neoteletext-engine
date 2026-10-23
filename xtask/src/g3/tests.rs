//! The G3 geometry's symmetry, rounding and stroke checks (decision E.50).

use super::geometry::Y;
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
