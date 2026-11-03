//! Writing a page from text in any language (REQ-CLI-010, decision E.82:
//! rdm REQ-95 and DEC-606): the writer is `neoteletext_formats::write_text`,
//! the one encoder the server uses too (REQ-2148, REQ-2214), and this is
//! its name for `neottx write-text`.

pub use neoteletext_formats::write_text::{Choice, Level, SETS, WriteError, Written, set_names, write_text};
