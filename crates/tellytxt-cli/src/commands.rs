//! The `ttx` commands (REQ-CLI-001).

use crate::output::image::{Frame, Scale, iterm2_inline, png, ppm, raster};
use crate::output::text::{ColourDepth, MosaicStyle, ansi, plain};
use crate::sandbox::{self, Policy};
use crate::session::Session;
use crate::terminal::{self, Display, TextArea};
use crate::{Common, GraphicsChoice, RasterArgs, exit, input};
use clap::ValueEnum;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use tellytxt_core::acq::StoredPage;
use tellytxt_core::consts::{CELL_WIDTH, COLUMNS, ROWS, SIDE_PANEL_MAX_COLUMNS};
use tellytxt_core::policy::is_safe_file_name_byte;
use tellytxt_core::render::row_width;
use tellytxt_core::types::{PageAddress, SubCode};
use tellytxt_image::palette::Rgba;

/// A command's failure: an exit status and a message.
pub type Failure = (u8, String);

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

/// The pages a selection names.
fn selected(session: &Session, selection: Selection) -> Result<Vec<&StoredPage>, Failure> {
    match selection {
        Selection::All => Ok(session.pages().into_iter().filter(|page| page.key.address.page.is_decimal()).collect()),
        Selection::Page(address, subcode) => session
            .page(address, subcode)
            .map(|page| vec![page])
            .ok_or_else(|| (exit::NOT_FOUND, format!("page {} not found", String::from_utf8_lossy(&address.ascii())))),
    }
}

fn sandbox(common: &Common, writable: Option<&Path>) -> Result<(), Failure> {
    sandbox::apply(Policy { writable_directory: writable, network: false, readable: &[] }, common.no_sandbox)
        .map_err(|message| (exit::INPUT, message))
}

/// `ttx list`.
pub fn list(common: &Common) -> Result<(), Failure> {
    let session = input::load(common)?;
    sandbox(common, None)?;
    let mut out = std::io::stdout().lock();
    let pages = session.pages();
    let mut index = 0;
    while index < pages.len() {
        let address = pages[index].key.address;
        let subpages: Vec<&&StoredPage> =
            pages[index..].iter().take_while(|page| page.key.address == address).collect();
        let receptions: u32 = subpages.iter().map(|page| page.receptions).sum();
        let errors: u32 = subpages.iter().map(|page| page.errors).sum();
        let codes: Vec<String> = subpages.iter().map(|page| format!("{:04X}", page.key.subcode.value())).collect();
        let _ = writeln!(
            out,
            "{}  sub-pages {:>3}  receptions {:>5}  errors {:>5}  [{}]",
            String::from_utf8_lossy(&address.ascii()),
            subpages.len(),
            receptions,
            errors,
            codes.join(" ")
        );
        index += subpages.len();
    }
    let stats = session.decoder().statistics();
    let _ = writeln!(
        out,
        "\n{} page(s); packets per magazine 1-8: {:?}; address failures {}; parity failures {}; triplet failures {}; discarded {}",
        pages.len(),
        [1, 2, 3, 4, 5, 6, 7, 0].map(|m| stats.per_magazine[m]),
        stats.address_failures,
        stats.parity_failures,
        stats.triplet_failures,
        stats.discarded
    );
    Ok(())
}

fn mosaic_style(legacy: bool) -> MosaicStyle {
    if legacy { MosaicStyle::Legacy } else { MosaicStyle::Unicode16 }
}

/// `ttx text`.
pub fn text(common: &Common, page: &str, reveal: bool, legacy_mosaics: bool) -> Result<(), Failure> {
    let selection = parse_selection(page)?;
    let session = input::load_for(common, selection.page())?;
    sandbox(common, None)?;
    let mut out = std::io::stdout().lock();
    for page in selected(&session, selection)? {
        let grid = session.grid(page, None);
        if selection == Selection::All {
            let _ = writeln!(
                out,
                "=== {}:{:04X}",
                String::from_utf8_lossy(&page.key.address.ascii()),
                page.key.subcode.value()
            );
        }
        let _ = out.write_all(plain(&grid, reveal, mosaic_style(legacy_mosaics)).as_bytes());
    }
    Ok(())
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

/// `ttx show`.
pub fn show(
    common: &Common,
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
    let session = input::load_for(common, selection.page())?;
    let is_terminal = std::io::stdout().is_terminal();
    let (display, reason) = terminal::detect(display(graphics), is_terminal);
    if let (Some(reason), true) = (reason, common.verbose > 0) {
        eprintln!("ttx: using text output: {reason}");
    }
    let frame = Frame::still(session.decoder().config(), raster_args.reveal);
    // Measure the terminal before the sandbox closes /dev/tty.
    let area = if display == Display::Iterm2 { terminal::text_area() } else { None };
    sandbox(common, None)?;
    let page = selected(&session, selection)?[0];
    let grid = session.grid(page, None);
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

/// `ttx view`.
pub fn view(
    common: &Common,
    raster_args: &RasterArgs,
    graphics: GraphicsChoice,
    legacy_mosaics: bool,
    colours8: bool,
) -> Result<(), Failure> {
    let (mut session, mut source) = input::open_live(common)?;
    let (display, _) = terminal::detect(display(graphics), std::io::stdout().is_terminal());
    let area = if display == Display::Iterm2 { terminal::text_area() } else { None };
    // Fit the widest page, 56 columns with side panels (decision E.49).
    let widest = (COLUMNS + usize::from(SIDE_PANEL_MAX_COLUMNS)) * CELL_WIDTH;
    let height = ROWS * Frame::still(session.decoder().config(), false).options.row_height();
    let scale = scale_for(raster_args, area, (widest, height));
    sandbox::apply(Policy { writable_directory: None, network: false, readable: &[] }, common.no_sandbox)
        .map_err(|message| (exit::INPUT, message))?;
    let depth = if colours8 { ColourDepth::Eight } else { ColourDepth::TrueColour };
    crate::view::run(&mut session, source.as_mut(), display, scale, depth, mosaic_style(legacy_mosaics))
        .map_err(|error| (exit::INPUT, error.to_string()))
}

/// A file name for a page, from validated parts only (REQ-SEC-010).
fn page_file_name(page: &StoredPage, extension: &str) -> String {
    let name =
        format!("P{}-{:04X}.{extension}", String::from_utf8_lossy(&page.key.address.ascii()), page.key.subcode.value());
    name.bytes().filter(|&byte| is_safe_file_name_byte(byte)).map(char::from).collect()
}

/// `ttx render`.
pub fn render(
    common: &Common,
    page: &str,
    output: &Path,
    raster_args: &RasterArgs,
    image: ImageFormat,
) -> Result<(), Failure> {
    let selection = parse_selection(page)?;
    let session = input::load_for(common, selection.page())?;
    let frame = Frame::still(session.decoder().config(), raster_args.reveal);
    let scale = scale_for(raster_args, None, (0, 0));
    let pages = selected(&session, selection)?;
    let directory: PathBuf = if selection == Selection::All {
        std::fs::create_dir_all(output).map_err(|error| (exit::INPUT, format!("{}: {error}", output.display())))?;
        output.to_path_buf()
    } else {
        output
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
    };
    sandbox(common, Some(&directory))?;
    let extension = match image {
        ImageFormat::Png => "png",
        ImageFormat::Ppm => "ppm",
    };
    for page in pages {
        let grid = session.grid(page, None);
        let picture = match image {
            ImageFormat::Png => Rgba::TRANSPARENT,
            ImageFormat::Ppm => Rgba::opaque(0, 0, 0),
        };
        let image_data = raster(&grid, frame, scale, picture);
        let bytes = match image {
            ImageFormat::Png => png(&image_data),
            ImageFormat::Ppm => ppm(&image_data, Rgba::opaque(0, 0, 0)),
        }
        .map_err(|error| (exit::INPUT, error.to_string()))?;
        let target = if selection == Selection::All {
            directory.join(page_file_name(page, extension))
        } else {
            output.to_path_buf()
        };
        std::fs::write(&target, bytes).map_err(|error| (exit::INPUT, format!("{}: {error}", target.display())))?;
    }
    Ok(())
}

/// `ttx service-info`.
pub fn service_info(common: &Common) -> Result<(), Failure> {
    let session = input::load(common)?;
    sandbox(common, None)?;
    let Some(info) = session.service() else {
        return Err((exit::NOT_FOUND, "no packet 8/30 in the input".into()));
    };
    println!("format: {}", if info.format2 { 2 } else { 1 });
    if let Some((address, subcode)) = info.initial_page {
        println!("initial page: {} sub-code {:04X}", String::from_utf8_lossy(&address.ascii()), subcode.value());
    }
    if let Some(network) = info.network_id {
        println!("network identification: {network:04X}");
    }
    if let Some(time) = info.utc {
        println!(
            "date and time (UTC): {:04}-{:02}-{:02} {:02}:{:02}:{:02} (refers to the next second boundary)",
            time.year, time.month, time.day, time.hour, time.minute, time.second
        );
    }
    if let Some(offset) = info.offset_half_hours {
        println!(
            "local offset: {}{}:{:02}",
            if offset < 0 { '-' } else { '+' },
            offset.unsigned_abs() / 2,
            (offset.unsigned_abs() % 2) * 30
        );
    }
    // REQ-SVC-005: the core decodes the message with the Latin G0 set.
    let status: String = tellytxt_core::charset::latin_text(&info.status)
        .iter()
        .map(|&c| if tellytxt_core::policy::is_safe_output_char(c) { c } else { ' ' })
        .collect();
    println!("status: {}", status.trim_end());
    Ok(())
}

/// `ttx services`.
pub fn services(common: &Common) -> Result<(), Failure> {
    input::services(common)
}

/// `ttx convert`.
pub fn convert(common: &Common, output: &Path, to: ConvertTarget, page: Option<&str>) -> Result<(), Failure> {
    let filter = match page {
        Some(text) => match parse_selection(text)? {
            Selection::Page(address, _) => Some(address),
            Selection::All => None,
        },
        None => None,
    };
    input::convert(common, output, to, filter)
}

/// `ttx slice`.
pub fn slice(common: &Common, output: &Path, preset: &str) -> Result<(), Failure> {
    input::slice(common, output, preset)
}

/// `ttx check`.
pub fn check(files: &[PathBuf], descriptor: Option<&Path>) -> Result<(), Failure> {
    input::check(files, descriptor)
}

#[cfg(test)]
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
