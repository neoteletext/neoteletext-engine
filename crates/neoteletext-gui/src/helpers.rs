//! A tab's helper processes (REQ-SEC-026, `docs/desktop-browser.md` section 3.1): the
//! fetcher, which holds the network, and the decoder, which holds the
//! site's pages. Both are this program run again, confined before their
//! first instruction on macOS, and confining themselves elsewhere.
//!
//! Everything they send arrives on one channel, already checked by the
//! strict frame parser; a helper that breaks the protocol is cut off.

use neoteletext_app::pipe::{self, Frame};
use neoteletext_app::sandbox::{Helper, helper_command};
use neoteletext_browser::Location;
use std::io;
use std::net::IpAddr;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};

/// The hidden command that runs the fetcher.
pub const FETCH_COMMAND: &str = "fetch";
/// The hidden command that runs a decoder.
pub const DECODE_COMMAND: &str = "decode";
/// Tells a helper that the program starting it has confined it.
pub const SANDBOXED_BY_PARENT: &str = "--sandboxed-by-parent";
/// Turns every sandbox off, for debugging only.
pub const NO_SANDBOX: &str = "--no-sandbox";
/// Allows one local-network address (REQ-SEC-006).
pub const ALLOW_LOCAL: &str = "--allow-local";

/// How helpers are started.
#[derive(Clone, Debug)]
pub struct Options {
    /// This program, run again as each helper.
    pub program: PathBuf,
    /// Local-network addresses the fetcher may reach (REQ-SEC-006).
    pub allow_local: Vec<IpAddr>,
    /// Run without sandboxes.
    pub no_sandbox: bool,
}

/// What a helper said or did.
#[derive(Debug)]
pub enum Message {
    /// A frame from the fetcher.
    Fetcher(Frame),
    /// A frame from the decoder.
    Decoder(Frame),
    /// The fetcher ended, or broke the protocol.
    FetcherEnded,
    /// The decoder ended, or broke the protocol.
    DecoderEnded,
}

/// Called from the readers' threads when a message arrives, to wake the
/// interface.
pub type Wake = Arc<dyn Fn() + Send + Sync>;

struct Process {
    child: Child,
    input: ChildStdin,
}

impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A tab's helpers.
pub struct Helpers {
    decoder: Process,
    fetcher: Option<Process>,
    /// What they send.
    pub messages: Receiver<Message>,
}

fn start(
    options: &Options,
    helper: Helper,
    arguments: &[String],
    sender: Sender<Message>,
    wake: Wake,
    wrap: fn(Frame) -> Message,
    ended: Message,
) -> Result<Process, String> {
    let (mut command, by_parent) = helper_command(&options.program, helper)?;
    command.args(arguments);
    if by_parent {
        command.arg(SANDBOXED_BY_PARENT);
    }
    if options.no_sandbox {
        command.arg(NO_SANDBOX);
    }
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit());
    let mut child = command.spawn().map_err(|error| format!("cannot start a helper: {error}"))?;
    let input = child.stdin.take().ok_or("no pipe to the helper")?;
    let mut output = child.stdout.take().ok_or("no pipe from the helper")?;
    std::thread::spawn(move || {
        // Every frame is validated by pipe::read; a bad one ends the link.
        while let Ok(Some(frame)) = pipe::read(&mut output) {
            if sender.send(wrap(frame)).is_err() {
                return;
            }
            wake();
        }
        let _ = sender.send(ended);
        wake();
    });
    Ok(Process { child, input })
}

impl Helpers {
    /// Starts the decoder, and the fetcher when the location has one.
    pub fn start(options: &Options, location: &Location, wake: &Wake) -> Result<Self, String> {
        let (sender, messages) = mpsc::channel();
        let decoder = start(
            options,
            Helper::Decoder,
            &[DECODE_COMMAND.to_string()],
            sender.clone(),
            wake.clone(),
            Message::Decoder,
            Message::DecoderEnded,
        )?;
        let fetcher = match location.fetcher_address() {
            Some(address) => {
                let mut arguments = vec![FETCH_COMMAND.to_string(), address];
                for local in &options.allow_local {
                    arguments.push(ALLOW_LOCAL.to_string());
                    arguments.push(local.to_string());
                }
                Some(start(
                    options,
                    Helper::Fetcher,
                    &arguments,
                    sender,
                    wake.clone(),
                    Message::Fetcher,
                    Message::FetcherEnded,
                )?)
            }
            None => None,
        };
        Ok(Self { decoder, fetcher, messages })
    }

    /// Sends a frame to the decoder.
    pub fn send_decoder(&mut self, frame: &Frame) -> io::Result<()> {
        pipe::write(&mut self.decoder.input, frame)
    }

    /// Sends a frame to the fetcher, if there is one.
    pub fn send_fetcher(&mut self, frame: &Frame) -> io::Result<()> {
        match &mut self.fetcher {
            Some(fetcher) => pipe::write(&mut fetcher.input, frame),
            None => Ok(()),
        }
    }
}
