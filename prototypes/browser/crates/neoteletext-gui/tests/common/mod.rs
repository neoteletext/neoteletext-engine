//! The local test sites of `prototypes/browser/tests/sites.py`, shared by the
//! end-to-end and window tests.
#![allow(dead_code, clippy::unwrap_used)]

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

/// The running sites and their ports.
pub struct Sites {
    child: Child,
    /// Each site's port.
    pub ports: HashMap<String, u16>,
}

impl Drop for Sites {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Starts the sites, or skips when Python is missing.
pub fn sites() -> Option<Sites> {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/sites.py");
    let mut child = match Command::new("python3").arg("-I").arg(&script).stdout(Stdio::piped()).spawn() {
        Ok(child) => child,
        Err(error) => {
            eprintln!("SKIPPED: python3 is needed for the test sites: {error}");
            return None;
        }
    };
    let mut line = String::new();
    BufReader::new(child.stdout.take()?).read_line(&mut line).ok()?;
    let mut ports = HashMap::new();
    for part in line.trim().trim_matches(|c| c == '{' || c == '}').split(',') {
        let (name, port) = part.split_once(':')?;
        ports.insert(name.trim().trim_matches('"').to_string(), port.trim().parse().ok()?);
    }
    Some(Sites { child, ports })
}

impl Sites {
    /// An address on a site.
    pub fn address(&self, name: &str, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.ports[name])
    }
}
