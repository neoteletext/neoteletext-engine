//! Object invocation (clauses 13.3 to 13.5, 13.8): default objects, the
//! limits of Annex A.8, and accepting an object for the walk.

use super::super::{Frame, Walker};

use super::super::Attrs;
use super::super::planes::{Plane, scan_index, type_index};
use crate::consts::{
    OBJECT_BODY_TRIPLETS, OBJECT_INVOCATIONS_PER_TYPE, OBJECTS_OPEN_AT_ONCE, OBJECTS_OPENED_IN_SUCCESSION,
};
use crate::objects::{self, ObjectRef};
use crate::packet::{ObjectKind, ObjectNumber, ObjectSource};

impl<'s> Walker<'_, 's> {
    /// A default object of REQ-OBJ-040, found and accepted, with origin and
    /// active position row 0, column 0 (clause 13.8).
    pub(super) fn default_object(&mut self, default: objects::DefaultInvocation) -> Option<Frame<'s>> {
        // Annex A.8 item 1: default objects count toward the limits.
        if !self.within_limits(default.kind, (0, 0)) {
            return None;
        }
        let inputs = self.inputs;
        let found = objects::find_public(
            inputs.lookup,
            Some(default.page),
            inputs.page,
            default.kind,
            default.number,
            inputs.level,
        );
        let Ok(object) = found else {
            self.report.faults.absent_objects = self.report.faults.absent_objects.saturating_add(1);
            return None;
        };
        self.accept(object, (0, 0), (0, 0))
    }

    /// An Object Invocation (REQ-OBJ-001 to REQ-OBJ-008).
    pub(super) fn invoke(
        &mut self,
        frame: &Frame<'s>,
        source: ObjectSource,
        kind: ObjectKind,
        number: ObjectNumber,
        (row_offset, column_offset): (u8, u8),
    ) -> Option<Frame<'s>> {
        let inputs = self.inputs;
        // REQ-OBJ-008; a local object only from the page's own data.
        let allowed = frame.plane.may_invoke(kind)
            && match source {
                ObjectSource::Illegal => false,
                ObjectSource::Local => frame.body.is_local(),
                ObjectSource::Public | ObjectSource::Global => true,
            };
        if !allowed {
            self.report.faults.forbidden_invocations = self.report.faults.forbidden_invocations.saturating_add(1);
            return None;
        }
        let invocation = frame.position();
        if !self.within_limits(kind, invocation) {
            return None;
        }
        let found = if source == ObjectSource::Local {
            objects::find_local(inputs.page, kind, number, inputs.level)
        } else {
            objects::find_public(
                inputs.lookup,
                inputs.links.object_page(source),
                inputs.page,
                kind,
                number,
                inputs.level,
            )
        };
        let Ok(object) = found else {
            self.report.faults.absent_objects = self.report.faults.absent_objects.saturating_add(1);
            return None;
        };
        let origin = (
            invocation.0.saturating_add(usize::from(row_offset)),
            invocation.1.saturating_add(usize::from(column_offset)),
        );
        self.accept(object, origin, invocation)
    }

    /// The limits of Annex A.8 (REQ-OBJ-020), checked before an object is
    /// looked for or scanned, so that the work stays bounded whatever the
    /// data: at most 127 invocations of each type, every permitted
    /// invocation counted whether or not its object is found; at most 15
    /// objects open at the invocation's position; at most 15 objects opened
    /// in succession without anything displayable written.
    fn within_limits(&mut self, kind: ObjectKind, invocation: (usize, usize)) -> bool {
        let start = scan_index(invocation.0, invocation.1);
        let open = self.open.as_slice().iter().filter(|&&(first, last)| first <= start && start <= last).count();
        let Some(count) = self.invoked.get_mut(type_index(kind)) else { return false };
        if *count >= OBJECT_INVOCATIONS_PER_TYPE
            || open >= OBJECTS_OPEN_AT_ONCE
            || self.successive >= OBJECTS_OPENED_IN_SUCCESSION
        {
            self.report.faults.limited_invocations = self.report.faults.limited_invocations.saturating_add(1);
            return false;
        }
        *count = count.saturating_add(1);
        true
    }

    /// Scans an object and makes its walk record (REQ-OBJ-006): it is open
    /// from the invocation to the last position it addresses (Annex A.8).
    fn accept(
        &mut self,
        object: ObjectRef<'s>,
        origin: (usize, usize),
        invocation: (usize, usize),
    ) -> Option<Frame<'s>> {
        let scanned = objects::prescan(&object, self.inputs.page);
        let Ok(extent) = scanned else {
            self.report.steps = self.report.steps.saturating_add(u32::from(OBJECT_BODY_TRIPLETS));
            self.report.faults.absent_objects = self.report.faults.absent_objects.saturating_add(1);
            return None;
        };
        self.report.steps = self.report.steps.saturating_add(u32::from(extent.end.saturating_sub(object.first)));
        let start = scan_index(invocation.0, invocation.1);
        let end = extent.last.map_or(start, |(row, column)| {
            scan_index(origin.0.saturating_add(usize::from(row)), origin.1.saturating_add(usize::from(column)))
        });
        let _ = self.open.push((start, end.max(start)));
        self.successive = self.successive.saturating_add(1);
        self.report.objects = self.report.objects.saturating_add(1);
        let slot = self.next_slot;
        self.next_slot = self.next_slot.saturating_add(1);
        Some(Frame {
            plane: Plane::of(object.kind),
            body: object.body,
            cursor: object.first,
            end: extent.end,
            local_data: false,
            origin,
            row: 0,
            column: 0,
            slot,
            designation: None,
            passive: Attrs::PASSIVE,
        })
    }
}
