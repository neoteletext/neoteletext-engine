//! Everything the neoteletext browser decides, with no operating-system
//! access (REQ-BRW-001, `prototypes/browser/docs/desktop-browser.md` section 3.5): what an address in the
//! address bar means, which channel a page belongs to, each tab's history,
//! when the tuning screen must be shown, and the rows the receiver draws
//! itself (row 0, the tuning screen, the start page, the contents view).
//!
//! The windowing front end, the processes and the storage belong to the
//! program; the program asks this crate what to do and to draw. Every rule
//! of trust lives here, once, so that every front end behaves alike.

pub mod charsets;
pub mod contents_view;
pub mod fonts;
pub mod location;
pub mod navigation;
pub mod pages;
pub mod rows;
pub mod saved;
pub mod tuning;

pub use location::{AddressError, Destination, Location};
pub use navigation::{Entry, Navigation};
/// The date a contents file gives a page (REQ-WEB-025).
pub use neoteletext_web::contents::Date as ContentsDate;
pub use tuning::{Tuning, Why};
