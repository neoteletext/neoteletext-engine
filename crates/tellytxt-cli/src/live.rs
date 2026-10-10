//! Live sources: the zxnet feeds (REQ-INP-022) and web teletext services
//! (area WEB), through the sans-IO web module.

use crate::commands::Failure;
use crate::exit;
use crate::view::Source;
use std::path::Path;

/// Opens a live source.
pub fn open(address: &str) -> Result<Box<dyn Source>, Failure> {
    Err((exit::USAGE, format!("{address}: live sources are not wired into ttx yet")))
}

/// The short names of the services a descriptor file lists, for
/// `ttx check --descriptor` (REQ-CLI-006).
pub fn descriptor_service_names(path: &Path) -> Result<Vec<Vec<u8>>, Failure> {
    let json = std::fs::read(path).map_err(|error| (exit::INPUT, format!("{}: {error}", path.display())))?;
    let base = tellytxt_web::url::Url::parse("https://localhost/descriptor.json")
        .map_err(|error| (exit::INPUT, format!("{error:?}")))?;
    let descriptor = tellytxt_web::descriptor::Descriptor::parse(&json, &base)
        .map_err(|error| (exit::INPUT, format!("{}: {error:?}", path.display())))?;
    Ok(descriptor.services().map(|service| service.short_name.as_bytes().to_vec()).collect())
}
