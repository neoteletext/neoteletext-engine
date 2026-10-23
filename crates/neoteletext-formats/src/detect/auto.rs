//! The automatic reader: it holds the input while detecting, then passes it
//! and everything after to the chosen reader.

#[cfg(feature = "pages")]
use neoteletext_core::packet::PacketMeta;

#[cfg(feature = "pages")]
use crate::encode::encode_page;
use crate::item::{Diagnostic, DiagnosticKind, Item, Reader};
use crate::text::copy_prefix;

use super::{AnyReader, AutoConfig, DETECTION_BYTES, Detection, Format, detect};
#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)] // As for AnyReader.
enum State {
    Detecting,
    Active(AnyReader),
    Failed,
}

/// A reader that detects the format and then reads it (REQ-INP-005). It
/// holds up to [`DETECTION_BYTES`] while detecting, then passes them and
/// everything after to the chosen reader.
#[derive(Clone, Debug)]
pub struct AutoReader {
    config: AutoConfig,
    window: [u8; DETECTION_BYTES],
    len: usize,
    state: State,
}

impl AutoReader {
    /// A reader. With `config.format` set, detection is skipped.
    #[must_use]
    pub fn new(config: AutoConfig) -> Self {
        let state = config.format.map_or(State::Detecting, |format| State::Active(AnyReader::new(format, 0, &config)));
        Self { config, window: [0; DETECTION_BYTES], len: 0, state }
    }

    /// Feeds the active reader, encoding pages into packets when asked.
    // Without `pages` no reader produces pages, so the configuration has
    // nothing to say.
    #[cfg_attr(not(feature = "pages"), allow(unused_variables))]
    fn pass(state: &mut State, config: &AutoConfig, bytes: &[u8], finish: bool, out: &mut dyn FnMut(Item<'_>)) {
        let State::Active(reader) = state else { return };
        #[cfg(feature = "pages")]
        let mut encoding = |item: Item<'_>| {
            out(item);
            if let (true, Item::Page(page)) = (config.encode_pages, item) {
                let meta = PacketMeta { source: config.source, ..PacketMeta::default() };
                encode_page(page, &mut |packet| out(Item::Packet { packet, meta }));
            }
        };
        #[cfg(feature = "pages")]
        let out: &mut dyn FnMut(Item<'_>) = &mut encoding;
        reader.feed(bytes, out);
        if finish {
            reader.finish(out);
        }
    }

    fn decide(&mut self, complete: bool, out: &mut dyn FnMut(Item<'_>)) {
        let held = self.window.get(..self.len).unwrap_or(&[]);
        match detect(held, complete) {
            Detection::Found { format, skip } => {
                out(Item::Detected(format));
                self.state = State::Active(AnyReader::new(format, skip, &self.config));
                // A T42 reader skips its own leading bytes.
                let start = if matches!(format, Format::T42(_)) { 0 } else { skip };
                let held = self.window.get(start..self.len).unwrap_or(&[]);
                Self::pass(&mut self.state, &self.config, held, false, out);
            }
            Detection::NeedMore => {}
            Detection::Unknown => {
                out(Item::Diagnostic(Diagnostic::at(0, DiagnosticKind::UnknownFormat)));
                self.state = State::Failed;
            }
        }
    }
}

impl Reader for AutoReader {
    fn feed(&mut self, bytes: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let mut rest = bytes;
        while matches!(self.state, State::Detecting) && !rest.is_empty() {
            let take = copy_prefix(self.window.get_mut(self.len..).unwrap_or(&mut []), rest);
            self.len = self.len.saturating_add(take);
            rest = rest.get(take..).unwrap_or(&[]);
            self.decide(false, out);
            if matches!(self.state, State::Detecting) && self.len == DETECTION_BYTES {
                // A full window decides; NeedMore cannot follow it.
                self.state = State::Failed;
            }
        }
        Self::pass(&mut self.state, &self.config, rest, false, out);
    }

    fn finish(&mut self, out: &mut dyn FnMut(Item<'_>)) {
        if matches!(self.state, State::Detecting) {
            if self.len == 0 {
                return;
            }
            self.decide(true, out);
        }
        Self::pass(&mut self.state, &self.config, &[], true, out);
    }
}
