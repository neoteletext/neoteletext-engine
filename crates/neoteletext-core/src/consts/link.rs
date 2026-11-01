//! The address of a link into a receiver, which no teletext standard covers.

/// What begins a link from another program, with a whole `http` or `https`
/// address after it (neoteletext-common's DEC-1666, DEC-1726). It lives here
/// because every program built on the engine reads or writes it, and this is
/// the one crate all of them have. The owner expects this name to change
/// (DEC-1726 item 5).
pub const LINK_SCHEME: &str = "neottx://";
