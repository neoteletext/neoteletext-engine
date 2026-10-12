//! The `ttx` commands (REQ-CLI-001).

use crate::output::image::{Frame, Scale, iterm2_inline, png, ppm, raster};
use crate::output::text::{ColourDepth, MosaicStyle, ansi, plain};
use crate::terminal::{self, Display, TextArea};
use crate::{GraphicsChoice, RasterArgs};
use clap::ValueEnum;
pub use neoteletext_app::Failure;
use neoteletext_app::input::{self, Options};
use neoteletext_app::sandbox::{self, Policy};
use neoteletext_app::session::Session;
use neoteletext_app::{convert as conversion, exit};
use neoteletext_core::acq::StoredPage;
use neoteletext_core::consts::ROWS;
use neoteletext_core::fsm::PageKey;
use neoteletext_core::level1::Grid;
use neoteletext_core::policy::is_safe_file_name_byte;
use neoteletext_core::render::row_width;
use neoteletext_core::types::{PageAddress, SubCode};
use neoteletext_formats::Item;
use neoteletext_image::palette::Rgba;
use std::fs::File;
use std::io::{IsTerminal, Write};
use std::net::IpAddr;
use std::path::{Path, PathBuf};

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

fn sandbox(options: &Options, writable: Option<&Path>) -> Result<(), Failure> {
    sandbox::apply(Policy { writable_directory: writable, network: false, readable: &[] }, options.no_sandbox)
        .map_err(|message| (exit::INPUT, message))
}

/// `ttx list`.
pub fn list(options: &Options) -> Result<(), Failure> {
    let mut session = input::load(options)?;
    sandbox(options, None)?;
    let mut out = std::io::stdout().lock();
    let pages = session.pages();
    let page_count = pages.len();
    let keys: Vec<PageKey> = pages.iter().map(|page| page.key).collect();
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
    // REQ-L25-051: enhancement faults are met only when a page is built, so
    // every page is built once to count them.
    let enhanced = session.decoder().config().level.is_enhanced();
    if enhanced {
        for key in keys {
            let _ = session.grid(key, None);
        }
    }
    let stats = session.decoder().statistics();
    let _ = writeln!(
        out,
        "\n{} page(s); packets per magazine 1-8: {:?}; address failures {}; parity failures {}; triplet failures {}; discarded {}",
        page_count,
        [1, 2, 3, 4, 5, 6, 7, 0].map(|m| stats.per_magazine[m]),
        stats.address_failures,
        stats.parity_failures,
        stats.triplet_failures,
        stats.discarded
    );
    if enhanced {
        let faults = &stats.enhancement;
        let _ = writeln!(
            out,
            "enhancement faults: damaged triplets {}; forbidden invocations {}; absent objects {}; limited invocations {}; discarded writes {}; ignored designations {}; undefined characters {}; DRCS missing {}; DRCS invalid {}",
            faults.damaged_triplets,
            faults.forbidden_invocations,
            faults.absent_objects,
            faults.limited_invocations,
            faults.discarded_writes,
            faults.ignored_designations,
            faults.undefined_characters,
            faults.drcs_missing,
            faults.drcs_invalid
        );
    }
    Ok(())
}

fn mosaic_style(legacy: bool) -> MosaicStyle {
    if legacy { MosaicStyle::Legacy } else { MosaicStyle::Unicode16 }
}

/// `ttx text`.
pub fn text(options: &Options, page: &str, reveal: bool, legacy_mosaics: bool, cells: bool) -> Result<(), Failure> {
    let selection = parse_selection(page)?;
    let mut session = input::load_for(options, selection.page())?;
    sandbox(options, None)?;
    let mut out = std::io::stdout().lock();
    for key in selected(&session, selection)? {
        let grid = grid_of(&mut session, key)?;
        if selection == Selection::All {
            let _ = writeln!(out, "=== {}:{:04X}", String::from_utf8_lossy(&key.address.ascii()), key.subcode.value());
        }
        let written =
            if cells { crate::output::cells::cells(&grid) } else { plain(&grid, reveal, mosaic_style(legacy_mosaics)) };
        let _ = out.write_all(written.as_bytes());
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
        eprintln!("ttx: using text output: {reason}");
    }
    let frame = Frame::still(session.decoder().config(), raster_args.reveal);
    // Measure the terminal before the sandbox closes /dev/tty.
    let area = if display == Display::Iterm2 { terminal::text_area() } else { None };
    sandbox(options, None)?;
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

/// `ttx view`.
#[cfg(feature = "view")]
pub fn view(
    options: &Options,
    raster_args: &RasterArgs,
    graphics: GraphicsChoice,
    legacy_mosaics: bool,
    colours8: bool,
) -> Result<(), Failure> {
    use neoteletext_core::consts::{CELL_WIDTH, COLUMNS, SIDE_PANEL_MAX_COLUMNS};
    let (mut session, mut source) = input::open_live(options)?;
    let (display, _) = terminal::detect(display(graphics), std::io::stdout().is_terminal());
    let area = if display == Display::Iterm2 { terminal::text_area() } else { None };
    // Fit the widest page, 56 columns with side panels (decision E.49).
    let widest = (COLUMNS + usize::from(SIDE_PANEL_MAX_COLUMNS)) * CELL_WIDTH;
    let height = ROWS * Frame::still(session.decoder().config(), false).options.row_height();
    let scale = scale_for(raster_args, area, (widest, height));
    sandbox::apply(Policy { writable_directory: None, network: false, readable: &[] }, options.no_sandbox)
        .map_err(|message| (exit::INPUT, message))?;
    let depth = if colours8 { ColourDepth::Eight } else { ColourDepth::TrueColour };
    crate::view::run(&mut session, source.as_mut(), display, scale, depth, mosaic_style(legacy_mosaics))
        .map_err(|error| (exit::INPUT, error.to_string()))
}

/// `ttx view` in a build without the viewer.
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

/// A file name for a page, from validated parts only (REQ-SEC-010).
fn page_file_name(key: PageKey, extension: &str) -> String {
    let name = format!("P{}-{:04X}.{extension}", String::from_utf8_lossy(&key.address.ascii()), key.subcode.value());
    name.bytes().filter(|&byte| is_safe_file_name_byte(byte)).map(char::from).collect()
}

/// `ttx render`.
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
    sandbox(options, Some(&directory))?;
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
        let image_data = raster(&grid, frame, scale, picture);
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
        std::fs::write(&target, bytes).map_err(|error| (exit::INPUT, format!("{}: {error}", target.display())))?;
    }
    Ok(())
}

/// `ttx service-info`.
pub fn service_info(options: &Options) -> Result<(), Failure> {
    let session = input::load(options)?;
    sandbox(options, None)?;
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
    let status: String = neoteletext_core::charset::latin_text(&info.status)
        .iter()
        .map(|&c| if neoteletext_core::policy::is_safe_output_char(c) { c } else { ' ' })
        .collect();
    println!("status: {}", status.trim_end());
    Ok(())
}

/// `ttx services`: the teletext streams of a transport stream (REQ-INP-007).
pub fn services(options: &Options) -> Result<(), Failure> {
    let mut input = input::open(options)?;
    let mut reader = input::reader(options, false);
    let mut found = 0u32;
    let mut out = std::io::stdout().lock();
    input::pump(input.as_mut(), reader.as_mut(), &mut |item| match item {
        Item::TeletextEntry { program, pid, entry } => {
            found += 1;
            let _ = writeln!(
                out,
                "program {program:>5}  PID {pid:#06x}  language {}  {:?}  page {}",
                String::from_utf8_lossy(&entry.language),
                entry.kind,
                String::from_utf8_lossy(&entry.page.ascii())
            );
        }
        Item::SubtitlePage(page) => {
            let _ = writeln!(out, "default subtitle page: {}", String::from_utf8_lossy(&page.ascii()));
        }
        _ => {}
    })?;
    if found == 0 {
        return Err((exit::NOT_FOUND, "no teletext descriptors found".into()));
    }
    Ok(())
}

/// `ttx convert`.
pub fn convert(options: &Options, output: &Path, to: ConvertTarget, page: Option<&str>) -> Result<(), Failure> {
    let filter = match page {
        Some(text) => match parse_selection(text)? {
            Selection::Page(address, _) => Some(address),
            Selection::All => None,
        },
        None => None,
    };
    let target = match to {
        ConvertTarget::T42 => conversion::Target::T42,
        ConvertTarget::Tti => conversion::Target::Tti,
    };
    conversion::convert(options, output, target, filter)
}

/// `ttx slice`.
#[cfg(feature = "slice")]
pub fn slice(options: &Options, output: &Path, preset: &str) -> Result<(), Failure> {
    let counters = neoteletext_app::slice::slice(options, output, preset)?;
    eprintln!("{counters:?}");
    Ok(())
}

/// `ttx slice` in a build without the slicer.
#[cfg(not(feature = "slice"))]
pub fn slice(_options: &Options, _output: &Path, _preset: &str) -> Result<(), Failure> {
    Err(neoteletext_app::not_in_build("raw video slicing", "slice"))
}

/// `ttx fetch`, the fetcher process of live mode (REQ-SEC-018), which
/// `ttx` starts by running itself again.
#[cfg(feature = "live")]
pub fn fetch(address: &str, no_sandbox: bool, allow_local: &[IpAddr]) -> Result<(), Failure> {
    neoteletext_app::fetch::run(address, no_sandbox, false, allow_local)
}

/// `ttx fetch` in a build without live feeds.
#[cfg(not(feature = "live"))]
pub fn fetch(_address: &str, _no_sandbox: bool, _allow_local: &[IpAddr]) -> Result<(), Failure> {
    Err(neoteletext_app::not_in_build("the fetcher process of live mode", "live"))
}

/// The short names of the services a descriptor lists, for `--descriptor`.
#[cfg(feature = "live")]
fn descriptor_service_names(path: &Path) -> Result<Vec<Vec<u8>>, Failure> {
    neoteletext_app::live::descriptor_service_names(path)
}

/// Service descriptors are read by the web module, which a build without
/// live feeds leaves out.
#[cfg(not(feature = "live"))]
fn descriptor_service_names(_path: &Path) -> Result<Vec<Vec<u8>>, Failure> {
    Err(neoteletext_app::not_in_build("service descriptors (--descriptor)", "live"))
}

/// `ttx check`: the page checker (REQ-CLI-006). Exit status 4 on errors.
pub fn check(files: &[PathBuf], descriptor: Option<&Path>) -> Result<(), Failure> {
    use neoteletext_formats::check::{CheckOptions, Severity, check_page};
    let services: Vec<Vec<u8>> = match descriptor {
        Some(path) => descriptor_service_names(path)?,
        None => Vec::new(),
    };
    let names: Vec<&[u8]> = services.iter().map(Vec::as_slice).collect();
    let options = CheckOptions { services: descriptor.map(|_| names.as_slice()) };
    let mut errors = 0u32;
    for file in files {
        let mut input = File::open(file).map_err(|error| (exit::INPUT, format!("{}: {error}", file.display())))?;
        let mut reader = neoteletext_formats::tti::TtiReader::default();
        input::pump(&mut input, &mut reader, &mut |item| {
            if let Item::Page(page) = item {
                check_page(page, &options, &mut |finding| {
                    let severity = match finding.severity {
                        Severity::Error => {
                            errors += 1;
                            "error"
                        }
                        Severity::Warning => "warning",
                    };
                    println!(
                        "{}:{}: page {} sub-page {}: {severity}: {:?}",
                        file.display(),
                        finding.line,
                        String::from_utf8_lossy(&finding.page.ascii()),
                        finding.subpage,
                        finding.problem
                    );
                });
            }
        })?;
    }
    if errors > 0 { Err((exit::CHECK_ERRORS, format!("{errors} error(s)"))) } else { Ok(()) }
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
