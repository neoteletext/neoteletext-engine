//! Reading a descriptor file for `neottx check --descriptor` (REQ-CLI-006).

use crate::bounded_file::read_bounded;
use crate::{Failure, exit};
use neoteletext_web::channel::SETTINGS_FILE;
use neoteletext_web::consts::DEFAULT_DESCRIPTOR_LIMIT;
use neoteletext_web::descriptor::Descriptor;
use std::path::Path;

/// The short names of the services a descriptor file lists, for
/// `neottx check --descriptor` (REQ-CLI-006). The file's name says which
/// kind it is, as it does for a reader (REQ-WEB-018): a channel's own file,
/// whose members are all optional, or a service's descriptor.
pub fn descriptor_service_names(path: &Path) -> Result<Vec<Vec<u8>>, Failure> {
    let json = read_bounded(path, DEFAULT_DESCRIPTOR_LIMIT)?;
    let base = neoteletext_web::url::Url::parse("https://localhost/descriptor.json")
        .map_err(|error| (exit::INPUT, format!("{error:?}")))?;
    let parse = if path.file_name().is_some_and(|name| name == SETTINGS_FILE) {
        Descriptor::parse_channel
    } else {
        Descriptor::parse
    };
    let descriptor = parse(&json, &base).map_err(|error| (exit::INPUT, format!("{}: {error:?}", path.display())))?;
    Ok(descriptor.services().map(|service| service.short_name.as_bytes().to_vec()).collect())
}
