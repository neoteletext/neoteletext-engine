//! Fetches the real-broadcast corpus into `$TELLYTXT_CORPUS` and pins each
//! file's checksum (REQ-TST-022, REQ-SEC-022). Downloads use `curl` with
//! ranged requests; files already present with the pinned checksum are
//! kept. Missing sources are reported, not fatal.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const MANIFEST: &str = "tests/corpus-manifest.txt";
const UNPINNED: &str = "unpinned";

fn corpus_dir() -> Result<PathBuf, String> {
    std::env::var_os("TELLYTXT_CORPUS").map(PathBuf::from).ok_or_else(|| "set TELLYTXT_CORPUS to a directory".to_string())
}

fn sha256(path: &Path) -> Result<String, String> {
    let output = Command::new("shasum").args(["-a", "256"]).arg(path).output().map_err(|e| format!("shasum: {e}"))?;
    let text = String::from_utf8_lossy(&output.stdout);
    text.split_whitespace().next().map(str::to_string).ok_or_else(|| format!("no checksum for {}", path.display()))
}

/// Downloads every manifest entry, writing pins for unpinned entries back.
pub fn fetch(root: &Path) -> Result<(), String> {
    let directory = corpus_dir()?;
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let manifest = fs::read_to_string(root.join(MANIFEST)).map_err(|e| e.to_string())?;
    let mut updated = String::new();
    let mut failures = 0usize;
    for line in manifest.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let [name, bytes, pin, url] = fields.as_slice() else {
            updated.push_str(line);
            updated.push('\n');
            continue;
        };
        if line.starts_with('#') {
            updated.push_str(line);
            updated.push('\n');
            continue;
        }
        let target = directory.join(name);
        let have = target.exists() && sha256(&target).is_ok_and(|sum| sum == *pin);
        if !have {
            let mut command = Command::new("curl");
            command.args(["--fail", "--silent", "--show-error", "--location", "--max-time", "300", "-o"]).arg(&target);
            if *bytes != "0" {
                let last = bytes.parse::<u64>().map_err(|e| e.to_string())?.saturating_sub(1);
                command.arg("--range").arg(format!("0-{last}"));
            }
            let status = command.arg(url).status().map_err(|e| format!("curl: {e}"))?;
            if !status.success() {
                eprintln!("MISSING {name}: download failed from {url}");
                failures += 1;
                updated.push_str(line);
                updated.push('\n');
                continue;
            }
        }
        let sum = sha256(&target)?;
        if *pin != UNPINNED && sum != *pin {
            return Err(format!("{name}: checksum {sum} does not match the pinned {pin}"));
        }
        println!("ok {name} {sum}");
        updated.push_str(&format!("{name:<32} {bytes:>8} {sum} {url}\n"));
    }
    fs::write(root.join(MANIFEST), updated).map_err(|e| e.to_string())?;
    if failures > 0 { Err(format!("{failures} file(s) missing")) } else { Ok(()) }
}
