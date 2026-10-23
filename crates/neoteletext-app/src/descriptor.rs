//! Reading a descriptor file for `neottx check --descriptor` (REQ-CLI-006).

use crate::{Failure, exit};
use std::path::Path;

/// The short names of the services a descriptor file lists, for
/// `neottx check --descriptor` (REQ-CLI-006).
pub fn descriptor_service_names(path: &Path) -> Result<Vec<Vec<u8>>, Failure> {
    let json = std::fs::read(path).map_err(|error| (exit::INPUT, format!("{}: {error}", path.display())))?;
    let base = neoteletext_web::url::Url::parse("https://localhost/descriptor.json")
        .map_err(|error| (exit::INPUT, format!("{error:?}")))?;
    let descriptor = neoteletext_web::descriptor::Descriptor::parse(&json, &base)
        .map_err(|error| (exit::INPUT, format!("{}: {error:?}", path.display())))?;
    Ok(descriptor.services().map(|service| service.short_name.as_bytes().to_vec()).collect())
}
