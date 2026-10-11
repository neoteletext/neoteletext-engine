//! What the page area shows, as RGBA pixels: a decoder's screen with the
//! receiver's own row 0 drawn over its first row, or a page the receiver
//! draws itself (`docs/web-native-concept.md` section 4).
//!
//! Row 0 is drawn here from the browser's own knowledge on every page, so
//! a decoder, whatever it sends, never decides a single pixel of it.

use crate::tab::{Tab, View};
use neoteletext_app::pipe::Screen;
use neoteletext_app::present::{Scale, raster, receiver_grid};
use neoteletext_browser::Location;
use neoteletext_browser::pages;
use neoteletext_browser::rows::{Activity, Page, Row, blank, reported_digits, row0};
use neoteletext_core::config::Config;
use neoteletext_core::consts::{HEADER_DISPLAY_COLUMN, ROWS, STATUS_COLUMNS};
use neoteletext_core::glyphs::GlyphSet;
use neoteletext_core::render::{FlashPhase, RenderOptions};
use neoteletext_image::palette::Rgba;

/// Bytes per pixel of RGBA.
const RGBA: usize = 4;
/// The opaque black around and behind pages.
const BLACK: [u8; RGBA] = [0, 0, 0, u8::MAX];

/// A picture to show.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Picture {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Red, green, blue and alpha, row by row.
    pub rgba: Vec<u8>,
}

impl Picture {
    /// The height of one teletext row.
    #[must_use]
    pub const fn row_height(&self) -> usize {
        self.height / ROWS
    }
}

/// Renders a page the receiver draws itself.
#[must_use]
pub fn receiver(page: &Page, sets: &[GlyphSet]) -> Picture {
    let mut status = [0u8; STATUS_COLUMNS];
    status.copy_from_slice(&page[0][..STATUS_COLUMNS]);
    let grid = receiver_grid(page, status);
    let options = RenderOptions::for_level(&Config::default(), false, FlashPhase::STEADY);
    let image = raster(&grid, options, true, sets, Scale { x: 1, y: 1, aspect: false }, Rgba::opaque(0, 0, 0));
    let mut rgba = Vec::with_capacity(image.indices.len() * RGBA);
    for &index in &image.indices {
        let colour = image.palette.get(usize::from(index)).copied().unwrap_or(Rgba::opaque(0, 0, 0));
        rgba.extend_from_slice(&[colour.r, colour.g, colour.b, u8::MAX]);
    }
    Picture { width: image.width, height: image.height, rgba }
}

/// A decoder's screen as RGBA; the palette covers every index, as the
/// frame parser checked.
#[must_use]
pub fn screen(screen: &Screen) -> Picture {
    let mut rgba = Vec::with_capacity(screen.indices.len() * RGBA);
    for &index in &screen.indices {
        let [r, g, b, _] = screen.palette.get(usize::from(index)).copied().unwrap_or(BLACK);
        rgba.extend_from_slice(&[r, g, b, u8::MAX]);
    }
    Picture { width: usize::from(screen.width), height: usize::from(screen.height), rgba }
}

/// Draws `row` as row 0 over `picture`: the whole width of the first row
/// is cleared to black first, then the row is drawn centred.
pub fn overlay_row0(picture: &mut Picture, row: &Row, sets: &[GlyphSet]) {
    let mut page = [blank(); ROWS];
    page[0] = *row;
    let drawn = receiver(&page, sets);
    let lines = drawn.row_height().min(picture.row_height());
    for line in 0..lines {
        let start = line * picture.width * RGBA;
        if let Some(target) = picture.rgba.get_mut(start..start + picture.width * RGBA) {
            for pixel in target.as_chunks_mut::<RGBA>().0 {
                *pixel = BLACK;
            }
        }
    }
    let width = drawn.width.min(picture.width);
    let left = (picture.width - width) / 2;
    for line in 0..lines {
        let source = &drawn.rgba[line * drawn.width * RGBA..][..width * RGBA];
        let start = (line * picture.width + left) * RGBA;
        if let Some(target) = picture.rgba.get_mut(start..start + width * RGBA) {
            target.copy_from_slice(source);
        }
    }
}

/// The header text of a page without a web site behind it: what the
/// broadcast or file put in row 0's columns 8 to 39, as plain text.
fn broadcast_header(text: &str) -> String {
    text.lines().next().unwrap_or("").chars().skip(HEADER_DISPLAY_COLUMN + 1).collect::<String>().trim().to_string()
}

/// The receiver's row 0 for a tab showing a decoder's page.
#[must_use]
pub fn tab_row0(tab: &Tab, clock: Option<&str>) -> Row {
    let status = tab.status.as_ref();
    let activity = if status.is_some_and(|status| status.form) {
        Activity::Form
    } else {
        Activity::Page {
            digits: status.map_or([b'-'; 3], |status| reported_digits(&status.status)),
            unencrypted: tab.is_unencrypted(),
        }
    };
    let label = match &tab.location {
        Some(location @ (Location::Site { .. } | Location::Service { .. })) => location.label(),
        Some(_) => status.map(|status| broadcast_header(&status.text)).unwrap_or_default(),
        None => String::new(),
    };
    // A web site has the browser's clock; broadcast text brings its own.
    let clock =
        if matches!(tab.location, Some(Location::Site { .. } | Location::Service { .. })) { clock } else { None };
    row0(activity, &label, clock)
}

/// What a tab's page area shows now.
#[must_use]
pub fn for_tab(tab: &Tab, bookmarks: &[String], clock: Option<&str>, sets: &[GlyphSet]) -> Picture {
    match &tab.view {
        View::Start => receiver(&pages::start(bookmarks, clock), sets),
        View::Keys => receiver(&pages::keys(clock), sets),
        View::Tuning(tuning) => receiver(&tuning.page(clock), sets),
        View::Failed(title, detail) => receiver(&pages::error(title, detail, clock), sets),
        View::Page => {
            if let Some(decoded) = &tab.screen {
                let mut picture = screen(decoded);
                overlay_row0(&mut picture, &tab_row0(tab, clock), sets);
                picture
            } else {
                let label = tab.location.as_ref().map(Location::label).unwrap_or_default();
                receiver(&pages::loading(&label, clock), sets)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neoteletext_app::pipe::PageStatus;

    const SETS: [GlyphSet; 1] = [GlyphSet::original()];

    #[test]
    fn a_decoder_cannot_draw_row_0() {
        // A hostile screen: every pixel white, including row 0.
        let hostile =
            Screen { width: 480, height: 500, palette: vec![[255, 255, 255, 255]], indices: vec![0; 480 * 500] };
        let mut tab = Tab::new(1);
        tab.view = View::Page;
        tab.location = Some(Location::Site { origin: "https://example.org".into(), secure: true });
        tab.screen = Some(hostile.clone());
        tab.status = Some(PageStatus { status: *b"GO? bank", found: true, ..PageStatus::default() });
        let picture = for_tab(&tab, &[], Some("12:00:00"), &SETS);
        let honest_row0 = receiver(
            &{
                let mut page = [blank(); ROWS];
                page[0] = row0(Activity::Page { digits: *b"---", unencrypted: false }, "example.org", Some("12:00:00"));
                page
            },
            &SETS,
        );
        let row_bytes = picture.row_height() * picture.width * RGBA;
        assert_eq!(&picture.rgba[..row_bytes], &honest_row0.rgba[..row_bytes], "row 0 is the receiver's alone");
        assert_eq!(picture.rgba[row_bytes..row_bytes + RGBA], [255, 255, 255, 255], "row 1 is the page's");
    }

    #[test]
    fn row0_shows_the_site_or_the_broadcast_header() {
        let mut tab = Tab::new(1);
        tab.location = Some(Location::Site { origin: "http://127.0.0.1:8001".into(), secure: false });
        tab.status = Some(PageStatus { status: *b"P140    ", found: true, ..PageStatus::default() });
        let row = tab_row0(&tab, Some("12:00:00"));
        assert_eq!(row[0], neoteletext_core::consts::ALPHA_RED, "plain http is marked");
        assert_eq!(&row[1..4], b"140");
        assert_eq!(&row[9..23], b"127.0.0.1:8001");
        tab.location = Some(Location::File { path: "/tmp/a.t42".into() });
        tab.status = Some(PageStatus {
            status: *b"P100    ",
            found: true,
            text: "P100     CEEFAX 100 Mon 01 Jan  12:00/00\n".into(),
            ..PageStatus::default()
        });
        let row = tab_row0(&tab, Some("99:99:99"));
        let header = String::from_utf8_lossy(&row[8..]).into_owned();
        assert!(header.contains("CEEFAX 100") && !header.contains("99:99"), "{header}");
    }
}
