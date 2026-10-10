//! Objects and the pages that carry them (area OBJ, layer L3; clauses 10.5,
//! 10.6 and 13; decisions E.51 to E.53).
//!
//! This module finds objects for the enhancement processor, and is filled
//! by a later phase. Its role:
//!
//! - the links to enhancement pages: a page's own X/27/4 and X/27/5, which
//!   take precedence, and otherwise the Magazine Organization Table of its
//!   magazine at the level being presented (REQ-OBJ-030, REQ-OBJ-031,
//!   REQ-OBJ-032);
//! - finding a local object in the page's packets X/26 (REQ-OBJ-003), and a
//!   public or global object through the pointer tables of its object page
//!   (REQ-OBJ-004);
//! - checking each definition's type and usage (REQ-OBJ-005) and scanning
//!   its body before it runs (REQ-OBJ-006);
//! - the default objects of the table (REQ-OBJ-040).
//!
//! It reads stored pages only through the read-only lookup of the
//! acquisition layer, `acq::EnhancementLookup` (REQ-ACQ-020), and decodes
//! their bytes only with the packet layer's decoders: `ObjectPacket`,
//! `triplet_location`, `mot_position` and the `mot_*` functions, and
//! `TripletFunction` for every triplet (REQ-PKT-010, REQ-PKT-017,
//! REQ-PKT-018). It may use the foundation, `packet` and `acq` only
//! (`ARCHITECTURE.md` section 9).
