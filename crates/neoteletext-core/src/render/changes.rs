//! Change tracking: which rows of a grid changed since the last frame.

use super::ALL_ROWS;
use super::colours::resolved_colours;
use super::flash::FlashPhase;
use super::options::{ColourMode, RenderOptions};
use crate::consts::{ROWS, SIDE_PANEL_FIRST_COLUMN};
use crate::glyphs::{CharClass, class};
use crate::level1::{Cell, Glyph, Grid};

/// Change tracking (REQ-RND-019, REQ-RND-032): which rows changed since the
/// port last asked.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ChangeTracker {
    hashes: [u64; ROWS],
    reveal: bool,
    flash: FlashPhase,
    primed: bool,
}

impl Default for ChangeTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// FNV-1a parameters for row fingerprints.
const FNV_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;

fn mix(hash: u64, value: u64) -> u64 {
    (hash ^ value).wrapping_mul(FNV_PRIME)
}

/// An FNV-1a hasher for row fingerprints.
struct Fnv(u64);

impl core::hash::Hasher for Fnv {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = mix(self.0, u64::from(byte));
        }
    }
}

/// True when a flashing cell looks different in phases `old` and `new`
/// (REQ-RND-032): its rate changes phase, it can show dots, and the dots'
/// colour differs between the two phases.
fn flash_changes<const PANEL_ROWS: usize>(
    cell: &Cell,
    grid: &Grid<PANEL_ROWS>,
    row: usize,
    column: usize,
    (old, new): (FlashPhase, FlashPhase),
    (reveal, underline): (bool, bool),
) -> bool {
    let rate = cell.attributes.flash_rate();
    if !cell.flash || old.is_on(rate) == new.is_on(rate) {
        return false;
    }
    let shows_dots = match cell.glyph {
        Glyph::Space => false,
        Glyph::Mosaic { pattern, .. } => pattern != 0,
        Glyph::Char(character) => match class(character) {
            CharClass::Text => !cell.glyph.is_blank() || (underline && cell.attributes.underline()),
            CharClass::G3(_) => true,
        },
    };
    shows_dots
        && resolved_colours(cell, grid, row, column, old, reveal)
            != resolved_colours(cell, grid, row, column, new, reveal)
}

impl ChangeTracker {
    /// A tracker that reports every row as changed on first use.
    #[must_use]
    pub const fn new() -> Self {
        Self { hashes: [0; ROWS], reveal: false, flash: FlashPhase::STEADY, primed: false }
    }

    /// Rows that changed since the last call, as a bitmask (bit r for row
    /// r), for drawing with `options`. A row changes when its cells, its
    /// side-panel cells, its row colour or the page-wide presentation data
    /// change, and the row below a changed double-height row changes too.
    ///
    /// - In the palette mode (REQ-RND-023), a change of reveal marks the
    ///   rows holding flashing concealed cells; other flash and reveal
    ///   changes are palette changes only.
    /// - In the resolved mode (REQ-RND-032), a change of reveal marks the
    ///   rows holding concealed cells, and a change of flash phase marks
    ///   exactly the rows holding a cell that looks different in the two
    ///   phases.
    pub fn changed_rows<const PANEL_ROWS: usize>(&mut self, grid: &Grid<PANEL_ROWS>, options: RenderOptions) -> u32 {
        let reveal = options.reveal;
        let reveal_changed = reveal != self.reveal;
        let mut page = Fnv(FNV_OFFSET);
        core::hash::Hash::hash(
            &(grid.boxed_mode, grid.mix, grid.side_panels, grid.colour_map, grid.screen_colour),
            &mut page,
        );
        let page = core::hash::Hasher::finish(&page);
        let mut changed = 0u32;
        for (row, stored) in self.hashes.iter_mut().enumerate() {
            let panel_row = row.checked_sub(1).and_then(|index| grid.panels.get(index));
            let cells = grid.cells.get(row).into_iter().flat_map(|cells| cells.iter().enumerate()).chain(
                panel_row.into_iter().flat_map(|cells| {
                    cells.iter().enumerate().map(|(index, cell)| (index.saturating_add(SIDE_PANEL_FIRST_COLUMN), cell))
                }),
            );
            let mut hasher = Fnv(page);
            if let Some(cells) = grid.cells.get(row) {
                core::hash::Hash::hash(cells, &mut hasher);
            }
            core::hash::Hash::hash(&grid.row_colours.get(row), &mut hasher);
            if let Some(panel) = panel_row {
                core::hash::Hash::hash(panel, &mut hasher);
            }
            let hash = core::hash::Hasher::finish(&hasher);
            let sensitive = match options.colour {
                ColourMode::Palette => reveal_changed && cells.clone().any(|(_, cell)| cell.flash && cell.conceal),
                ColourMode::Resolved(flash) => {
                    (reveal_changed && cells.clone().any(|(_, cell)| cell.conceal))
                        || (flash != self.flash
                            && cells.clone().any(|(column, cell)| {
                                flash_changes(cell, grid, row, column, (self.flash, flash), (reveal, options.underline))
                            }))
                }
            };
            let bit = 1u32.checked_shl(row as u32).unwrap_or(0);
            if !self.primed || hash != *stored || sensitive {
                changed |= bit;
            }
            *stored = hash;
        }
        // REQ-RND-019: the row below a changed double-height row changes too.
        let doubled = changed & grid.double_height_rows;
        changed |= doubled << 1;
        self.reveal = reveal;
        self.flash = match options.colour {
            ColourMode::Resolved(flash) => flash,
            ColourMode::Palette => FlashPhase::STEADY,
        };
        self.primed = true;
        changed & ALL_ROWS
    }

    /// Forces a full redraw on the next call.
    pub fn invalidate(&mut self) {
        self.primed = false;
    }
}
