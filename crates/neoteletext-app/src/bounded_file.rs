//! Reading a whole file of bounded size (all input is untrusted: every
//! length is bounded before it is used).

use crate::{Failure, exit};
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Reads a whole file of at most `limit` bytes. No more than one byte over
/// the limit is ever read, whatever the file's length.
///
/// # Errors
/// A file that cannot be read, or that is larger than `limit`.
pub fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, Failure> {
    let failure = |error: std::io::Error| (exit::INPUT, format!("{}: {error}", path.display()));
    let file = File::open(path).map_err(failure)?;
    let mut bytes = Vec::new();
    let cap = u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1);
    file.take(cap).read_to_end(&mut bytes).map_err(failure)?;
    if bytes.len() > limit {
        return Err((exit::INPUT, format!("{}: larger than {limit} bytes", path.display())));
    }
    Ok(bytes)
}
