//! Self-sandboxing (REQ-SEC-017, decision E.33 layer 5).
//!
//! After opening its inputs and outputs, `neottx` restricts itself so that it
//! cannot open other files, start programs, or use the network outside web
//! mode. Linux uses Landlock and a seccomp filter (Phase 1). Other systems
//! are Phase 2: `neottx` says so and continues (decision 15 of
//! IMPLEMENTATION-LOG.md).

use std::path::Path;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(not(target_os = "linux"))]
mod unsupported;
#[cfg(not(target_os = "linux"))]
use unsupported as platform;

/// What the sandboxed process may still do.
#[derive(Clone, Copy, Debug)]
pub struct Policy<'a> {
    /// A directory the process may create and write files in.
    pub writable_directory: Option<&'a Path>,
    /// The process may make network connections (web mode fetcher).
    pub network: bool,
    /// Directories the process may read (the fetcher reads `/etc` to
    /// resolve names).
    pub readable: &'a [&'a Path],
}

/// Applies the sandbox. `allow_unsandboxed` is `--no-sandbox`.
pub fn apply(policy: Policy<'_>, allow_unsandboxed: bool) -> Result<(), String> {
    if allow_unsandboxed {
        eprintln!("neottx: running without a sandbox (--no-sandbox)");
        return Ok(());
    }
    platform::apply(policy)
}

/// The REQ-SEC-017 verification: after the sandbox is applied, opening a
/// file, starting a program and connecting to the network must all fail.
pub fn self_test() -> Result<(), (u8, String)> {
    apply(Policy { writable_directory: None, network: false, readable: &[] }, false)
        .map_err(|message| (crate::exit::INPUT, message))?;
    let file = std::fs::File::open("/etc/hostname").is_err();
    let program = std::process::Command::new("/bin/true").status().is_err();
    let network = std::net::TcpStream::connect(("1.1.1.1", 80)).is_err();
    println!("open file blocked: {file}\nstart program blocked: {program}\nnetwork blocked: {network}");
    if file && program && network {
        Ok(())
    } else {
        Err((crate::exit::INPUT, "the sandbox did not block everything".to_string()))
    }
}
