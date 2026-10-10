//! The enhancement processor of Levels 2.5 and 3.5 (area L25, layer L3;
//! decisions E.40 to E.50; REQ-FSM-011 and REQ-FSM-012).
//!
//! This module is filled by a later phase. It will run after the Level 1
//! grid of a page has been built, in place of the Level 1.5 pass, when the
//! presentation level is 2.5 or 3.5 (REQ-L25-001, REQ-PRF-006), and will:
//!
//! - walk the page's local enhancement data, the objects it invokes and the
//!   default objects, as the state machine of REQ-FSM-011 describes, with
//!   the objects found by `objects`;
//! - resolve each row from its Level 1 bytes and the events of the walk,
//!   per cell, as REQ-FSM-012 describes, writing characters and every
//!   attribute of `level1::Cell` and its `CellAttributes`;
//! - apply the page-level presentation data that `colour::resolve` builds
//!   (the Colour Map, screen and row colours, re-mapping, substitution and
//!   side panels: REQ-L25-030 to REQ-L25-041);
//! - put into the grid the DRCS patterns and colour tables that `drcs`
//!   decodes for the characters the cells use (REQ-DRC-007).
//!
//! It never writes row 0, columns 0 to 7 (REQ-L25-050), uses no allocation,
//! and bounds its work by the limits of Annex A.8 (REQ-OBJ-020). It reaches
//! stored enhancement pages only through `acq::EnhancementLookup`
//! (REQ-ACQ-020). It may use the foundation, `charset`, `level1`, `packet`,
//! `acq`, `colour`, `objects` and `drcs` (`ARCHITECTURE.md` section 9).
