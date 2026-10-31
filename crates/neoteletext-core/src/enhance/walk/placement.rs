//! Placing what a column triplet writes: characters, attributes and the
//! Modified G0 and G2 designation, at the active position.

use super::super::{Collector, Frame, Walker};

use super::super::AttrChange;
use super::super::attributes::{ATTRIBUTES, Flash, flash_of};
use super::super::characters::EnhChar;
use super::super::planes::Plane;
use crate::charset::{self, G0Set, G2Set};
use crate::consts::{COLUMNS, RECEIVER_COLUMNS, ROWS, TRIPLET_AT_SIGN_CODE, TRIPLET_FIRST_CHARACTER};
use crate::packet::TripletFunction;

impl<'s> Walker<'_, 's> {
    /// The cell of the active position, when an element may be placed there
    /// (REQ-OBJ-014, REQ-L25-050, REQ-1644): an element outside rows 0 to
    /// 24 or columns 0 to 39 is discarded, and the source continues.
    fn placeable(&mut self, frame: &Frame<'s>) -> Option<(usize, usize)> {
        let (row, column) = frame.position();
        let receiver = row == 0 && column < RECEIVER_COLUMNS;
        if row >= ROWS || column >= COLUMNS || receiver {
            self.report.faults.discarded_writes = self.report.faults.discarded_writes.saturating_add(1);
            return None;
        }
        Some((row, column))
    }

    /// A column triplet: its column, then its character or attribute.
    pub(super) fn column_triplet(&mut self, frame: &mut Frame<'s>, function: TripletFunction, out: &mut Collector) {
        // REQ-L25-004: reserved modes and Programme Delivery Control leave
        // the position; so do row triplets that reach here, which are
        // reserved or Programme Delivery Control, or restricted (REQ-L25-052,
        // REQ-OBJ-013).
        let Some(column) = function.active_column() else { return };
        frame.column = column;
        let mut change = AttrChange::default();
        let character = match function {
            TripletFunction::ForegroundColour { colour, .. } if ATTRIBUTES && colour.is_defined() => {
                change.foreground = Some(colour.index);
                None
            }
            TripletFunction::BackgroundColour { colour, .. } if ATTRIBUTES && colour.is_defined() => {
                change.background = Some(colour.index);
                None
            }
            TripletFunction::AdditionalFlash { mode, rate, reserved, .. } if ATTRIBUTES => {
                // The phases count from the absolute column, set below.
                let Some(flash) = flash_of(mode, rate, reserved, 0) else { return };
                change.flash = Some(flash);
                None
            }
            TripletFunction::DisplayAttributes { attributes, .. } if ATTRIBUTES => {
                change.display = Some(attributes);
                None
            }
            TripletFunction::ModifiedDesignation { designation, .. } => {
                let Some(pair) = self.designation(designation) else { return };
                if matches!(frame.plane, Plane::Adaptive | Plane::Passive) {
                    // Annex C.4 rule 2: to the end of the object.
                    frame.designation = Some(pair);
                    return;
                }
                change.designation = Some(pair);
                None
            }
            TripletFunction::G1Character { code, .. } => Some(EnhChar::G1(code)),
            TripletFunction::G3Character { code, .. } => Some(EnhChar::G3(code)),
            TripletFunction::G0Character { code, .. } => Some(EnhChar::G0 { code, diacritic: 0, at: false }),
            TripletFunction::G2Character { code, .. } => Some(EnhChar::G2(code)),
            TripletFunction::G0Diacritic { code, diacritic, .. } => {
                Some(EnhChar::G0 { code, diacritic, at: diacritic == 0 && code == TRIPLET_AT_SIGN_CODE })
            }
            // Reserved data, and functions not acted on (decision E.95):
            // the triplet only sets the column.
            _ => return,
        };
        // REQ-L25-004: a character code below 2/0 sets the column and writes
        // nothing.
        if let Some(EnhChar::G0 { code, .. } | EnhChar::G1(code) | EnhChar::G2(code) | EnhChar::G3(code)) = character
            && code < TRIPLET_FIRST_CHARACTER
        {
            return;
        }
        let Some((row, column)) = self.placeable(frame) else { return };
        // Annex A.8 item 3: something displayable was written.
        self.successive = 0;
        if frame.plane == Plane::Adaptive {
            out.touch(row, column, frame.slot);
        }
        if let Some(Flash::On { start, .. }) = change.flash.as_mut() {
            *start = column as u8;
        }
        match character {
            Some(character) => Self::character(frame, row, column, character, out),
            None => Self::attribute(frame, row, column, change, out),
        }
    }

    /// A Modified G0 and G2 designation (REQ-L25-012): `None` when reserved,
    /// or at Level 2.5 when it would make a third pair on the page.
    fn designation(&mut self, value: u8) -> Option<(G0Set, G2Set)> {
        let accepted = charset::modified_designation(value).filter(|&pair| {
            self.inputs.level.is_level35()
                || pair == self.inputs.default_sets
                || *self.extra_pair.get_or_insert(pair) == pair
        });
        if accepted.is_none() {
            self.report.faults.ignored_designations = self.report.faults.ignored_designations.saturating_add(1);
        } else {
            self.report.designations = self.report.designations.saturating_add(1);
        }
        accepted
    }

    fn character(frame: &Frame<'s>, row: usize, column: usize, character: EnhChar, out: &mut Collector) {
        let Some(slot) = out.slot(row, column) else { return };
        // Clause 13.2: within one plane the later write wins (Annex A.7.1).
        match frame.plane {
            Plane::Local => slot.local = Some(character),
            Plane::Active => slot.active = Some(character),
            Plane::Adaptive => slot.adaptive = Some((character, frame.designation)),
            Plane::Passive => slot.passive = Some((character, frame.designation, frame.passive)),
        }
    }

    fn attribute(frame: &mut Frame<'s>, row: usize, column: usize, change: AttrChange, out: &mut Collector) {
        if frame.plane == Plane::Passive {
            // REQ-OBJ-011: a Passive object's attributes are its own and
            // persist across rows until it changes them.
            frame.passive.apply(&change);
            return;
        }
        let Some(slot) = out.slot(row, column) else { return };
        if frame.plane == Plane::Adaptive {
            slot.adaptive_change = slot.adaptive_change.then(change);
            slot.adaptive_owner = Some(frame.slot);
        } else {
            slot.underlying = slot.underlying.then(change);
        }
    }
}
