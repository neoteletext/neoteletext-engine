//! `neottx render` (REQ-RND-007): one page, or all, as image files.

use super::font::read_table;
use super::io_failure;
use super::{ImageFormat, Selection, grid_of, page_file_name, parse_selection, sandbox_writable, scale_for, selected};
use crate::args::RasterArgs;
use crate::frame::Frame;
use crate::image::{png, ppm, raster_in};
use neoteletext_app::input::{self, Options};
use neoteletext_app::{Failure, exit};
use neoteletext_core::glyphs::conformance::FONT_SET_COUNT;
use neoteletext_core::glyphs::{GlyphSet, LoadedFont};
use neoteletext_image::palette::Rgba;
use std::path::{Path, PathBuf};

/// `neottx render`.
pub fn render(
    options: &Options,
    page: &str,
    output: &Path,
    raster_args: &RasterArgs,
    image: ImageFormat,
) -> Result<(), Failure> {
    let selection = parse_selection(page)?;
    let mut session = input::load_for(options, selection.page())?;
    let frame = Frame::still(session.decoder().config(), raster_args.reveal);
    let scale = scale_for(raster_args, None, (0, 0));
    let table = raster_args.font.as_deref().map(read_table).transpose()?;
    let loaded =
        table.as_deref().map(LoadedFont::new).transpose().map_err(|error| (exit::INPUT, format!("{error:?}")))?;
    let glyphs = GlyphSet::bedstead().with_fonts(loaded, [None; FONT_SET_COUNT]);
    let pages = selected(&session, selection)?;
    let directory: PathBuf = if selection == Selection::All {
        std::fs::create_dir_all(output).map_err(|error| io_failure(output, error))?;
        output.to_path_buf()
    } else {
        output
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
    };
    sandbox_writable(Some(&directory), options.no_sandbox)?;
    let extension = match image {
        ImageFormat::Png => "png",
        ImageFormat::Ppm => "ppm",
    };
    for key in pages {
        let grid = grid_of(&mut session, key)?;
        let picture = match image {
            ImageFormat::Png => Rgba::TRANSPARENT,
            ImageFormat::Ppm => Rgba::opaque(0, 0, 0),
        };
        let image_data = raster_in(&grid, frame, scale, picture, &glyphs);
        let bytes = match image {
            ImageFormat::Png => png(&image_data),
            ImageFormat::Ppm => ppm(&image_data, Rgba::opaque(0, 0, 0)),
        }
        .map_err(|error| (exit::INPUT, error.to_string()))?;
        let target = if selection == Selection::All {
            directory.join(page_file_name(key, extension))
        } else {
            output.to_path_buf()
        };
        std::fs::write(&target, bytes).map_err(|error| io_failure(&target, error))?;
    }
    Ok(())
}
