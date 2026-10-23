//! `neottx show` (REQ-CLI-001): one page in the terminal, as an iTerm2
//! image or as characters.

use super::{Selection, display, grid_of, mosaic_style, parse_selection, sandbox_closed, scale_for, selected};
use crate::ansi::{ColourDepth, ansi, plain};
use crate::args::{GraphicsChoice, RasterArgs};
use crate::frame::Frame;
use crate::image::{iterm2_inline, png, raster};
use crate::terminal::{self, Display};
use neoteletext_app::input::{self, Options};
use neoteletext_app::{Failure, exit};
use neoteletext_core::consts::ROWS;
use neoteletext_core::render::row_width;
use neoteletext_image::palette::Rgba;
use std::io::{IsTerminal, Write};

/// `neottx show`.
pub fn show(
    options: &Options,
    page: &str,
    raster_args: &RasterArgs,
    graphics: GraphicsChoice,
    legacy_mosaics: bool,
    colours8: bool,
) -> Result<(), Failure> {
    let selection = parse_selection(page)?;
    if selection == Selection::All {
        return Err((exit::USAGE, "show displays one page; use text or render for all".into()));
    }
    let mut session = input::load_for(options, selection.page())?;
    let is_terminal = std::io::stdout().is_terminal();
    let (display, reason) = terminal::detect(display(graphics), is_terminal);
    if let (Some(reason), true) = (reason, options.verbose > 0) {
        eprintln!("neottx: using text output: {reason}");
    }
    let frame = Frame::still(session.decoder().config(), raster_args.reveal);
    // Measure the terminal before the sandbox closes /dev/tty.
    let area = if display == Display::Iterm2 { terminal::text_area() } else { None };
    sandbox_closed(options.no_sandbox)?;
    let key = selected(&session, selection)?[0];
    let grid = grid_of(&mut session, key)?;
    let mut out = std::io::stdout().lock();
    match display {
        Display::Iterm2 => {
            // A page with side panels is wider (REQ-RND-033).
            let scale = scale_for(raster_args, area, (row_width(&grid), ROWS * frame.options.row_height()));
            let image = raster(&grid, frame, scale, Rgba::opaque(0, 0, 0));
            let encoded = png(&image).map_err(|error| (exit::INPUT, error.to_string()))?;
            let _ = out.write_all(iterm2_inline(&encoded, image.width, image.height).as_bytes());
            let _ = out.write_all(b"\n");
        }
        Display::Text => {
            let depth = if colours8 { ColourDepth::Eight } else { ColourDepth::TrueColour };
            let text = if is_terminal || graphics == GraphicsChoice::Text {
                ansi(&grid, raster_args.reveal, mosaic_style(legacy_mosaics), depth)
            } else {
                plain(&grid, raster_args.reveal, mosaic_style(legacy_mosaics))
            };
            let _ = out.write_all(text.as_bytes());
        }
    }
    Ok(())
}
