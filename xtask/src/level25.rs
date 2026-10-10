//! Writes the authored Level 2.5 and 3.5 test stream (REQ-TST-024) as
//! T42, and optionally as TTI files: one per page address and `all.tti`
//! with every page in the order of Annex B.7. The stream is generated
//! from `tellytxt_formats::author::streams`, so it is never stored in the
//! repository.

use std::fs;
use std::path::{Path, PathBuf};

use tellytxt_formats::author::streams;

/// The name of the TTI file that holds every page.
const ALL_PAGES: &str = "all.tti";

/// `cargo xtask level25-stream OUT.t42 [--tti DIR]`.
pub fn write(arguments: &[String]) -> Result<(), String> {
    let usage = || "usage: cargo xtask level25-stream OUT.t42 [--tti DIR]".to_string();
    let (output, tti) = match arguments {
        [output] => (PathBuf::from(output), None),
        [output, flag, directory] if flag == "--tti" => (PathBuf::from(output), Some(PathBuf::from(directory))),
        _ => return Err(usage()),
    };
    let stream = streams::stream().map_err(|error| format!("building the stream: {error:?}"))?;
    fs::write(&output, stream.t42()).map_err(|error| format!("{}: {error}", output.display()))?;
    println!("{}: {} packets, {} pages", output.display(), stream.packets().len(), stream.pages().len());
    if let Some(directory) = tti {
        write_tti(&stream, &directory)?;
    }
    Ok(())
}

fn write_tti(stream: &tellytxt_formats::author::Stream, directory: &Path) -> Result<(), String> {
    fs::create_dir_all(directory).map_err(|error| format!("{}: {error}", directory.display()))?;
    let files = stream.tti_files().map_err(|error| error.to_string())?;
    for (name, text) in &files {
        let path = directory.join(name);
        fs::write(&path, text).map_err(|error| format!("{}: {error}", path.display()))?;
    }
    let all = stream.tti().map_err(|error| error.to_string())?;
    let path = directory.join(ALL_PAGES);
    fs::write(&path, all).map_err(|error| format!("{}: {error}", path.display()))?;
    println!("{}: {} files and {ALL_PAGES}", directory.display(), files.len());
    Ok(())
}
