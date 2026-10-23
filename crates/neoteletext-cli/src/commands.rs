//! The `neottx` commands (REQ-CLI-001).

use crate::ansi::MosaicStyle;
use crate::args::{GraphicsChoice, RasterArgs};
use crate::image::Scale;
use crate::terminal::{self, Display, TextArea};
use clap::ValueEnum;
pub use neoteletext_app::Failure;
use neoteletext_app::exit;
use neoteletext_app::sandbox::{self, Policy};
use neoteletext_app::session::Session;
use neoteletext_core::fsm::PageKey;
use neoteletext_core::level1::Grid;
use neoteletext_core::policy::is_safe_file_name_byte;
use neoteletext_core::types::{PageAddress, SubCode};
use std::path::Path;

mod check;
mod contents;
mod convert;
mod fetch;
mod font;
#[cfg(feature = "keyboard")]
mod keyboard;
mod list;
#[cfg(feature = "keyboard")]
mod qr;
mod render;
mod service_info;
mod services;
mod show;
mod slice;
mod sound;
mod text;
mod view;
mod write_text;

pub use check::check;
pub use contents::contents;
pub use convert::convert;
pub use fetch::fetch;
pub use font::{convert_font, font_sets};
#[cfg(feature = "keyboard")]
pub use keyboard::{KeyboardOptions, keyboard};
pub use list::list;
#[cfg(feature = "keyboard")]
pub use qr::qr;
pub use render::render;
pub use service_info::service_info;
pub use services::services;
pub use show::show;
pub use slice::slice;
pub use sound::{PLAYER_PACKETS_PER_SECOND, SoundOptions, sound};
pub use text::text;
pub use view::view;
pub use write_text::write_text;

/// Image file formats (REQ-RND-007).
#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum ImageFormat {
    /// Portable Network Graphics with alpha.
    Png,
    /// Binary Portable Pixmap.
    Ppm,
}

/// Conversion targets (REQ-CLI-001 `convert`).
#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum ConvertTarget {
    /// T42 packets.
    T42,
    /// TTI page files.
    Tti,
}

/// What a page argument selects (REQ-CLI-003).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Selection {
    /// Every stored page.
    All,
    /// One page, optionally one sub-page.
    Page(PageAddress, SubCode),
}

impl Selection {
    /// The page to ask a live source for.
    #[must_use]
    pub const fn page(self) -> Option<PageAddress> {
        match self {
            Self::All => None,
            Self::Page(address, _) => Some(address),
        }
    }
}

/// Parses `100`, `100:0003`, `8FF` or `all` (REQ-CLI-003).
pub fn parse_selection(text: &str) -> Result<Selection, Failure> {
    if text.eq_ignore_ascii_case("all") {
        return Ok(Selection::All);
    }
    let usage = || (exit::USAGE, format!("not a page number: {text:?} (expected e.g. 100 or 100:0003)"));
    let (page, sub) = match text.split_once(':') {
        Some((page, sub)) => (page, Some(sub)),
        None => (text, None),
    };
    if page.len() != 3 {
        return Err(usage());
    }
    let mpp = u16::from_str_radix(page, 16).map_err(|_| usage())?;
    let address = PageAddress::from_mpp(mpp).ok_or_else(usage)?;
    let subcode = match sub {
        Some(sub) => SubCode::new(u16::from_str_radix(sub, 16).map_err(|_| usage())?),
        None => SubCode::ANY,
    };
    Ok(Selection::Page(address, subcode))
}

/// The pages a selection names, by key, so that building their grids may
/// update the session's statistics.
fn selected(session: &Session, selection: Selection) -> Result<Vec<PageKey>, Failure> {
    match selection {
        Selection::All => Ok(session
            .pages()
            .into_iter()
            .filter(|page| page.key.address.page.is_decimal())
            .map(|page| page.key)
            .collect()),
        Selection::Page(address, subcode) => session
            .page(address, subcode)
            .map(|page| vec![page.key])
            .ok_or_else(|| (exit::NOT_FOUND, format!("page {} not found", String::from_utf8_lossy(&address.ascii())))),
    }
}

/// The grid of a page that `selected` named.
fn grid_of(session: &mut Session, key: PageKey) -> Result<Grid, Failure> {
    session
        .grid(key, None)
        .ok_or_else(|| (exit::NOT_FOUND, format!("page {} not found", String::from_utf8_lossy(&key.address.ascii()))))
}

/// Applies the sandbox (REQ-SEC-017) with `writable` the one directory the
/// command may still write in; `no_sandbox` is `--no-sandbox`.
fn sandbox_writable(writable: Option<&Path>, no_sandbox: bool) -> Result<(), Failure> {
    sandbox::apply(Policy { writable_directory: writable, network: false, readable: &[] }, no_sandbox)
        .map_err(|message| (exit::INPUT, message))
}

/// Applies the sandbox with nothing left to write and no network: every
/// file the command needs is already open (REQ-SEC-017).
pub(crate) fn sandbox_closed(no_sandbox: bool) -> Result<(), Failure> {
    sandbox_writable(None, no_sandbox)
}

fn mosaic_style(legacy: bool) -> MosaicStyle {
    if legacy { MosaicStyle::Legacy } else { MosaicStyle::Unicode16 }
}

fn display(choice: GraphicsChoice) -> Option<Display> {
    match choice {
        GraphicsChoice::Auto => None,
        GraphicsChoice::Iterm2 => Some(Display::Iterm2),
        GraphicsChoice::Text => Some(Display::Text),
    }
}

/// The scale: the one asked for, or the largest that fits `area` for a page
/// of `width` by `height` pixels, or 1.
fn scale_for(raster_args: &RasterArgs, area: Option<TextArea>, (width, height): (usize, usize)) -> Scale {
    let factor = if raster_args.scale > 0 {
        raster_args.scale
    } else {
        terminal::fit_scale(area, width, height, raster_args.aspect)
    };
    Scale { x: factor, y: factor, aspect: raster_args.aspect }
}

/// The failure to read or write `path`: the input exit status and the
/// path with the error, as every command words it (REQ-CLI-005).
pub(crate) fn io_failure(path: &Path, error: impl std::fmt::Display) -> Failure {
    (exit::INPUT, format!("{}: {error}", path.display()))
}

/// A file name for a page, from validated parts only (REQ-SEC-010).
fn page_file_name(key: PageKey, extension: &str) -> String {
    let name = format!("P{}-{:04X}.{extension}", String::from_utf8_lossy(&key.address.ascii()), key.subcode.value());
    name.bytes().filter(|&byte| is_safe_file_name_byte(byte)).map(char::from).collect()
}

/// A page number 100 to 8FF, as `--page` gives it to the commands that
/// write a page or its sound.
#[cfg(any(feature = "keyboard", feature = "sound", feature = "write-text"))]
fn page_number(text: &str) -> Result<u16, Failure> {
    u16::from_str_radix(text, 16)
        .ok()
        .filter(|&mpp| PageAddress::from_mpp(mpp).is_some())
        .ok_or_else(|| (exit::USAGE, format!("{text}: not a page number 100 to 8FF")))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "a test")]
mod tests {
    use super::*;

    #[test]
    fn page_selection() {
        assert_eq!(parse_selection("all"), Ok(Selection::All));
        assert_eq!(parse_selection("100"), Ok(Selection::Page(PageAddress::from_mpp(0x100).unwrap(), SubCode::ANY)));
        assert_eq!(
            parse_selection("8FF:0003"),
            Ok(Selection::Page(PageAddress::from_mpp(0x8FF).unwrap(), SubCode::new(3)))
        );
        assert!(parse_selection("900").is_err());
        assert!(parse_selection("10").is_err());
        assert!(parse_selection("1g0").is_err());
    }
}
