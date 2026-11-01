//! `neottx html FOLDER` (neoteletext-common's REQ-1725 item 3, DEC-1694 rule
//! 7): writes a static channel's pages for a web browser into its folder, so
//! that a shared address leads somewhere that explains itself and not to the
//! host's 404. `index.html` is the channel's small page, and `NNN/index.html`
//! a page's. A page shows nothing of itself unless the channel opts in by
//! naming it with `--public`; then it carries what a public page of a live
//! channel carries (REQ-1728), from the one builder both use.
//!
//! The channel's readers ask for its fixed file names, so these files change
//! nothing they do, and `NNN/` is never read as a channel (REQ-1725 item 4).

use super::contents::{Folder, Found, pages_of, read_folder, skipped_files};
use super::{io_failure, sandbox_writable};
use crate::image::png_picture;
use neoteletext_app::input::Options;
use neoteletext_app::present::{MosaicStyle, cell_text};
use neoteletext_app::{Failure, exit};
use neoteletext_core::level1::{Glyph, Grid};
use neoteletext_core::types::PageAddress;
use neoteletext_image::raster::picture_size;
use neoteletext_web::channel::{SETTINGS_FILE, parse_address};
use neoteletext_web::consts::DEFAULT_DESCRIPTOR_LIMIT;
use neoteletext_web::descriptor::Descriptor;
use neoteletext_web::map::Place;
use neoteletext_web::small_page::{PublicPage, channel_page, private_page, public_page};
use neoteletext_web::url::Url;
use std::collections::BTreeSet;
use std::path::Path;

/// The name every static host serves at a folder (REQ-1725 item 3).
const INDEX_FILE: &str = "index.html";
/// The rows of a page that say something: 1 to 23, between the header and
/// the prompts of row 24 (EN 300 706 clause 9.4).
const TEXT_ROWS: usize = 23;

/// The pages named with `--public`.
fn public_pages(public: &[String]) -> Result<BTreeSet<PageAddress>, Failure> {
    public
        .iter()
        .map(|text| match Place::read(text) {
            Some(Place::Page { page, subcode: None, parameters: None }) => Ok(page),
            _ => Err((exit::USAGE, format!("--public takes a page, 100 to 8FF, not {text}"))),
        })
        .collect()
}

/// The channel's name, from its entry file, read to its reader's limit.
fn channel_name(folder: &Path, address: &str) -> Result<String, Failure> {
    let path = folder.join(SETTINGS_FILE);
    let bytes = std::fs::read(&path).map_err(|error| io_failure(&path, error))?;
    if bytes.len() > DEFAULT_DESCRIPTOR_LIMIT {
        return Err((exit::INPUT, format!("{}: over {DEFAULT_DESCRIPTOR_LIMIT} bytes", path.display())));
    }
    let base = Url::parse(address).map_err(|error| (exit::USAGE, format!("{address}: {error:?}")))?;
    let descriptor = Descriptor::parse_channel(&bytes, &base)
        .map_err(|error| (exit::INPUT, format!("{}: {error:?}", path.display())))?;
    Ok(descriptor.name.as_str().to_owned())
}

/// The words of rows 1 to 23 of `grid`, one space between them: what a page
/// says. Mosaics are pictures, and row 24 is its prompts.
fn page_text(grid: &Grid) -> String {
    let mut text = String::new();
    for cells in grid.cells.iter().skip(1).take(TEXT_ROWS) {
        for cell in cells {
            if matches!(cell.glyph, Glyph::Mosaic { .. }) {
                text.push(' ');
            } else {
                cell_text(cell, grid, false, MosaicStyle::Unicode16, &mut text);
            }
        }
        text.push(' ');
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// What a page opted in shows.
fn public_html(
    folder: &mut Folder,
    name: &str,
    place: &str,
    page: PageAddress,
    found: &Found,
) -> Result<String, Failure> {
    let refused = || (exit::NOT_FOUND, format!("page {} not found", String::from_utf8_lossy(&page.ascii())));
    let session = &mut folder.loaded.get_mut(found.file).ok_or_else(refused)?.session;
    let key = session
        .pages()
        .iter()
        .find(|stored| stored.key.address == page)
        .map(|stored| stored.key)
        .ok_or_else(refused)?;
    let grid = session.grid(key, None).ok_or_else(refused)?;
    let picture = png_picture(&grid).map_err(|error| (exit::INPUT, error.to_string()))?;
    let (width, height) = picture_size();
    Ok(public_page(
        name,
        place,
        &PublicPage {
            title: &found.title,
            text: &page_text(&grid),
            canonical: place,
            picture: Some((&picture, width, height)),
        },
    ))
}

/// `neottx html FOLDER --address ADDRESS [--public PAGE]...`.
pub fn html(options: &Options, address: &str, public: &[String]) -> Result<(), Failure> {
    let folder = Path::new(&options.input);
    parse_address(address).map_err(|error| (exit::USAGE, format!("{address}: not a channel's address: {error:?}")))?;
    let public = public_pages(public)?;
    let name = channel_name(folder, address)?;
    let mut read = read_folder(options, |_| true)?;
    sandbox_writable(Some(folder), options.no_sandbox)?;
    let pages = pages_of(&mut read);
    if let Some(page) = public.iter().find(|page| !pages.contains_key(page)) {
        return Err((
            exit::NOT_FOUND,
            format!("--public names page {}, which the folder does not hold", String::from_utf8_lossy(&page.ascii())),
        ));
    }
    let write = |path: &Path, text: String| std::fs::write(path, text).map_err(|error| io_failure(path, error));
    write(&folder.join(INDEX_FILE), channel_page(&name, address))?;
    for (page, found) in &pages {
        let number = String::from_utf8_lossy(&page.ascii()).into_owned();
        let place = format!("{address}{number}");
        let html = if public.contains(page) {
            public_html(&mut read, &name, &place, *page, found)?
        } else {
            private_page(&name, &place)
        };
        let page_folder = folder.join(&number);
        std::fs::create_dir_all(&page_folder).map_err(|error| io_failure(&page_folder, error))?;
        write(&page_folder.join(INDEX_FILE), html)?;
    }
    skipped_files(&read)
}
