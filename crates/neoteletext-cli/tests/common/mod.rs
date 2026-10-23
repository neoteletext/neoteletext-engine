//! What the integration tests of `neottx` share: running the program, a
//! folder of a test's own, and a WAVE file to read.
#![allow(dead_code, reason = "each integration test uses only some of the helpers")]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Runs the `neottx` this package built with `arguments`.
pub fn neottx(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_neottx")).args(arguments).output().expect("run neottx")
}

/// A fresh folder of the test's own: tests run in parallel.
pub fn folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).expect("make the test folder");
    folder
}

/// A mono 16-bit WAVE file of `samples` at `rate`.
pub fn wave(samples: &[i16], rate: u32) -> Vec<u8> {
    let data: Vec<u8> = samples.iter().flat_map(|sample| sample.to_le_bytes()).collect();
    let mut bytes = b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0".to_vec();
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 2).to_le_bytes());
    bytes.extend_from_slice(&[2, 0, 16, 0]);
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes());
    bytes.extend_from_slice(&data);
    bytes
}
