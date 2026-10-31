//! Starting the fetcher process and reading its frames on a thread of
//! their own (REQ-SEC-018).

use super::Live;
use crate::pipe::{self, Frame};
use crate::source::{Source, Wake};
use crate::{Failure, exit};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::sync::mpsc;

/// Starts the fetcher process for `address`. Must run before sandboxing:
/// starting a program is blocked afterwards.
pub fn open(
    address: &str,
    no_sandbox: bool,
    allow_local: &[std::net::IpAddr],
    proxy: Option<&str>,
) -> Result<Box<dyn Source>, Failure> {
    let address = crate::input::link_address(address).map_err(|message| (exit::USAGE, message))?;
    let program = std::env::current_exe().map_err(|error| (exit::INPUT, error.to_string()))?;
    let mut command = Command::new(program);
    command.arg("fetch").arg(address).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit());
    if no_sandbox {
        command.arg("--no-sandbox");
    }
    for address in allow_local {
        command.arg("--allow-local").arg(address.to_string());
    }
    if let Some(proxy) = proxy {
        command.arg("--proxy").arg(proxy);
    }
    let mut child = command.spawn().map_err(|error| (exit::INPUT, format!("cannot start the fetcher: {error}")))?;
    let requests = child.stdin.take().ok_or((exit::INPUT, "no pipe to the fetcher".to_string()))?;
    let mut output = child.stdout.take().ok_or((exit::INPUT, "no pipe from the fetcher".to_string()))?;
    let (sender, frames) = mpsc::channel();
    let live = Live::new(Some(child), Box::new(requests), frames);
    let wake = live.wake.clone();
    std::thread::spawn(move || read_frames(&mut output, &sender, &wake));
    Ok(Box::new(live))
}

/// Reads the fetcher's frames until its pipe ends, passing each on and
/// waking the viewer, if it asked to be woken, as it comes.
pub(super) fn read_frames(output: &mut dyn std::io::Read, sender: &mpsc::Sender<Frame>, wake: &OnceLock<Wake>) {
    // Every frame is validated by pipe::read; a bad one ends the link.
    while let Ok(Some(frame)) = pipe::read(output) {
        if sender.send(frame).is_err() {
            return;
        }
        if let Some(wake) = wake.get() {
            wake();
        }
    }
    // The end is news too: the viewer sees the fetcher gone.
    if let Some(wake) = wake.get() {
        wake();
    }
}
