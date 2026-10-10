//! Web mode against the local test server (REQ-WEB-013): the whole path
//! from descriptor to decoded page, through the fetcher process.
#![allow(missing_docs)]

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

struct Server {
    child: Child,
    port: u16,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn server() -> Option<Server> {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/web-server/server.py");
    let mut child = match Command::new("python3").arg("-I").arg(&script).arg("0").stdout(Stdio::piped()).spawn() {
        Ok(child) => child,
        Err(error) => {
            eprintln!("SKIPPED: python3 is needed for the test server: {error}");
            return None;
        }
    };
    let mut line = String::new();
    BufReader::new(child.stdout.take()?).read_line(&mut line).ok()?;
    let port = line.trim().parse().ok()?;
    Some(Server { child, port })
}

fn ttx(arguments: &[&str]) -> (bool, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ttx")).args(arguments).output().expect("run ttx");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn page_is_fetched_from_a_web_service() {
    let Some(server) = server() else { return };
    let url = format!("http://127.0.0.1:{}/service.json", server.port);
    let (ok, text, errors) = ttx(&["text", &url, "100", "--allow-local", "127.0.0.1", "--duration", "15"]);
    assert!(ok, "ttx failed: {errors}");
    assert!(text.contains("TELLYTXT TEST SERVICE"), "{text}");
    assert!(text.contains("Contact form"), "{text}");
    // The TTI Link Extension row drawn from the LN lines (REQ-NAV-011).
    let row24 = text.lines().nth(24).unwrap_or_default();
    assert!(row24.contains("Weather") && row24.contains("Contact"), "row 24: {row24:?}");
}

#[test]
fn local_addresses_are_blocked_unless_named() {
    let Some(server) = server() else { return };
    let url = format!("http://127.0.0.1:{}/service.json", server.port);
    let (ok, _, errors) = ttx(&["text", &url, "100", "--duration", "5"]);
    assert!(!ok);
    assert!(errors.contains("Blocked"), "REQ-SEC-006: {errors}");
}

#[test]
fn faults_do_not_crash_the_receiver() {
    let Some(server) = server() else { return };
    for fault in ["drop", "big", "media", "loop", "ambiguous"] {
        // A descriptor address that misbehaves: ttx must fail cleanly.
        let url = format!("http://127.0.0.1:{}/fault/{fault}", server.port);
        let output = Command::new(env!("CARGO_BIN_EXE_ttx"))
            .args(["text", &url, "100", "--allow-local", "127.0.0.1", "--duration", "5"])
            .output()
            .expect("run ttx");
        assert_eq!(output.status.code(), Some(1), "fault {fault}: {}", String::from_utf8_lossy(&output.stderr));
    }
}

#[test]
fn viewer_fills_and_submits_a_form() {
    let Some(server) = server() else { return };
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/web-server/drive_viewer.py");
    let output = Command::new("python3")
        .arg("-I")
        .arg(script)
        .arg(env!("CARGO_BIN_EXE_ttx"))
        .arg(server.port.to_string())
        .output()
        .expect("run the viewer driver");
    let report = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{report}");
}

/// REQ-WEB-009: the driver starts and stops its own server, so that the
/// network goes away under a running viewer.
#[test]
fn viewer_works_offline_and_recovers() {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/web-server");
    let output = match Command::new("python3")
        .arg("-I")
        .arg(directory.join("drive_offline.py"))
        .arg(env!("CARGO_BIN_EXE_ttx"))
        .arg(directory.join("server.py"))
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            eprintln!("SKIPPED: python3 is needed for the offline test: {error}");
            return;
        }
    };
    let report = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{report}");
}
