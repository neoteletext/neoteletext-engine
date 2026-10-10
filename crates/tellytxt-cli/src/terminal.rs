//! Terminal capabilities: graphics protocol detection (REQ-RND-029,
//! decision 17 of IMPLEMENTATION-LOG.md) and size.

use tellytxt_core::consts::PAGE_WIDTH;

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

/// The largest whole-number scale at which a page of `page_height` lines
/// fits the terminal's text area, keeping one text row free for the
/// prompt. At least 1.
#[must_use]
pub fn fit_scale(page_height: usize, aspect: bool) -> usize {
    let Ok(size) = crossterm::terminal::window_size() else { return 1 };
    let (width, height) = (usize::from(size.width), usize::from(size.height));
    if width == 0 || height == 0 || size.rows == 0 {
        return 1;
    }
    let row_height = height / usize::from(size.rows);
    let usable_height = height.saturating_sub(row_height);
    let page_height = tellytxt_image::scale::scaled_size(PAGE_WIDTH, page_height, 1, 1, aspect).1;
    let by_width = width / PAGE_WIDTH;
    let by_height = usable_height / page_height.max(1);
    by_width.min(by_height).max(1)
}
