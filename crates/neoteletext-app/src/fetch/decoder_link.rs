//! The pipe to the decoder process: frames out on standard output, and a
//! thread reading its requests on standard input (decision 26 of
//! IMPLEMENTATION-LOG.md).

use super::wakes::Wake;
use crate::pipe::{self, Frame};
use neoteletext_core::types::{PageAddress, SubCode};
use std::sync::mpsc::Sender;

pub(super) fn send(frame: &Frame) -> std::io::Result<()> {
    let mut out = std::io::stdout().lock();
    pipe::write(&mut out, frame)
}

/// Reads the requests sent to the fetcher: page requests, links to other
/// services and keys. Anything else is dropped. The loop is woken by each,
/// and by the pipe's end.
pub(super) fn requests(wake: Sender<Wake>) {
    std::thread::spawn(move || {
        let mut input = std::io::stdin().lock();
        while let Ok(Some(frame)) = pipe::read(&mut input) {
            let wanted = matches!(frame, Frame::Request(..) | Frame::Follow(..) | Frame::Input(_));
            if wanted && wake.send(Wake::Request(frame)).is_err() {
                return;
            }
        }
        let _ = wake.send(Wake::Ended);
    });
}

pub(super) fn page_request(mpp: u16, sub: u16) -> Option<(PageAddress, Option<SubCode>)> {
    PageAddress::from_mpp(mpp).map(|address| (address, (sub != u16::MAX).then(|| SubCode::new(sub))))
}
