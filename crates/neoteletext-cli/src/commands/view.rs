//! `neottx view` (REQ-CLI-004): the interactive viewer, or its refusal in a
//! build without it.

use crate::args::{GraphicsChoice, RasterArgs};
use neoteletext_app::Failure;
use neoteletext_app::input::Options;
#[cfg(feature = "view")]
use {
    super::{display, mosaic_style, sandbox_closed, scale_for},
    crate::ansi::ColourDepth,
    crate::frame::Frame,
    crate::terminal::{self, Display},
    neoteletext_app::exit,
    neoteletext_app::input,
    neoteletext_core::consts::ROWS,
    std::io::IsTerminal,
};

/// `neottx view`.
#[cfg(feature = "view")]
pub fn view(
    options: &Options,
    raster_args: &RasterArgs,
    graphics: GraphicsChoice,
    legacy_mosaics: bool,
    colours8: bool,
) -> Result<(), Failure> {
    use neoteletext_core::consts::PAGE_WIDTH;
    let opening = input::opening(&options.input).map_err(|message| (exit::USAGE, message))?;
    let (mut session, mut source) = input::open_live(options)?;
    let (display, _) = terminal::detect(display(graphics), std::io::stdout().is_terminal());
    let area = if display == Display::Iterm2 { terminal::text_area() } else { None };
    let height = ROWS.saturating_mul(Frame::still(session.decoder().config(), false).options.row_height());
    let scale = scale_for(raster_args, area, (PAGE_WIDTH, height));
    sandbox_closed(options.no_sandbox)?;
    let depth = if colours8 { ColourDepth::Eight } else { ColourDepth::TrueColour };
    crate::view::run(&mut session, source.as_mut(), opening, display, scale, depth, mosaic_style(legacy_mosaics))
        .map_err(|error| (exit::INPUT, error.to_string()))
}

/// `neottx view` in a build without the viewer.
#[cfg(not(feature = "view"))]
pub fn view(
    _options: &Options,
    _raster_args: &RasterArgs,
    _graphics: GraphicsChoice,
    _legacy_mosaics: bool,
    _colours8: bool,
) -> Result<(), Failure> {
    Err(neoteletext_app::not_in_build("the interactive viewer", "view"))
}
