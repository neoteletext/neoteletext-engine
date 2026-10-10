//! Terminal capabilities: graphics protocol detection (REQ-RND-029,
//! decision 17 of IMPLEMENTATION-LOG.md) and size.

/// How a page is shown in the terminal.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Display {
    /// iTerm2 inline images.
    Iterm2,
    /// Characters with ANSI colour.
    Text,
}

/// The display to use, and the reason when graphics were not chosen.
#[must_use]
pub fn detect(forced: Option<Display>, is_terminal: bool) -> (Display, Option<&'static str>) {
    if let Some(display) = forced {
        return (display, None);
    }
    if !is_terminal {
        return (Display::Text, Some("output is not a terminal"));
    }
    if std::env::var_os("TMUX").is_some() || std::env::var_os("STY").is_some() {
        return (Display::Text, Some("inside tmux or GNU screen: graphics may not pass through"));
    }
    let iterm = std::env::var("TERM_PROGRAM").is_ok_and(|value| value == "iTerm.app")
        || std::env::var("LC_TERMINAL").is_ok_and(|value| value == "iTerm2");
    if iterm { (Display::Iterm2, None) } else { (Display::Text, Some("terminal is not iTerm2")) }
}

/// The terminal's text area in pixels, measured before the sandbox closes
/// the terminal device.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TextArea {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Text rows.
    pub rows: usize,
}

/// The text area, if the terminal reports its size in pixels.
#[cfg(feature = "view")]
#[must_use]
pub fn text_area() -> Option<TextArea> {
    let size = crossterm::terminal::window_size().ok()?;
    let area =
        TextArea { width: usize::from(size.width), height: usize::from(size.height), rows: usize::from(size.rows) };
    (area.width > 0 && area.height > 0 && area.rows > 0).then_some(area)
}

/// A build without the viewer has no terminal library to measure with,
/// so `show` draws images at scale 1 unless `--scale` names another.
#[cfg(not(feature = "view"))]
#[must_use]
pub const fn text_area() -> Option<TextArea> {
    None
}

/// The largest whole-number scale at which a page of `page_width` by
/// `page_height` pixels fits the text area, keeping one text row free for
/// the prompt (REQ-RND-029). At least 1. Pages with side panels are wider
/// (REQ-RND-033).
#[must_use]
pub fn fit_scale(area: Option<TextArea>, page_width: usize, page_height: usize, aspect: bool) -> usize {
    let Some(area) = area else { return 1 };
    let row_height = area.height / area.rows;
    let usable_height = area.height.saturating_sub(row_height);
    let (page_width, page_height) = tellytxt_image::scale::scaled_size(page_width, page_height, 1, 1, aspect);
    let by_width = area.width / page_width.max(1);
    let by_height = usable_height / page_height.max(1);
    by_width.min(by_height).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// REQ-RND-029: the scale fits the wider pages of REQ-RND-033 too.
    #[test]
    fn scale_fits_width_and_height() {
        let area = Some(TextArea { width: 1500, height: 1300, rows: 50 });
        assert_eq!(fit_scale(area, 480, 500, false), 2);
        assert_eq!(fit_scale(area, 672, 500, false), 2);
        assert_eq!(fit_scale(area, 864, 500, false), 1);
        assert_eq!(fit_scale(area, 480, 500, true), 2);
        assert_eq!(fit_scale(None, 480, 500, false), 1);
        assert_eq!(fit_scale(Some(TextArea { width: 100, height: 100, rows: 5 }), 480, 500, false), 1);
    }
}
