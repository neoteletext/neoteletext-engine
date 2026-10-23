//! `neottx check` (REQ-CLI-006): the page checker, with the service
//! descriptor its links are resolved against.

use neoteletext_app::input;
use neoteletext_app::{Failure, exit};
use neoteletext_formats::Item;
use std::fs::File;
use std::path::{Path, PathBuf};

/// The short names of the services a descriptor lists, for `--descriptor`.
#[cfg(feature = "live")]
fn descriptor_service_names(path: &Path) -> Result<Vec<Vec<u8>>, Failure> {
    neoteletext_app::descriptor::descriptor_service_names(path)
}

/// Service descriptors are read by the web module, which a build without
/// live feeds leaves out.
#[cfg(not(feature = "live"))]
fn descriptor_service_names(_path: &Path) -> Result<Vec<Vec<u8>>, Failure> {
    Err(neoteletext_app::not_in_build("service descriptors (--descriptor)", "live"))
}

/// `neottx check`: the page checker (REQ-CLI-006). Exit status 4 on errors.
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
