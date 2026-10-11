//! Helpers shared by the unit tests.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

use std::boxed::Box;
use std::vec::Vec;

use neoteletext_core::packet::{Packet, PacketMeta};

use crate::item::{Diagnostic, Item, Reader};
use crate::page::PageFile;

/// An [`Item`] that owns its page.
#[derive(Clone, PartialEq, Debug)]
pub enum Owned {
    /// A packet.
    Packet(Packet, PacketMeta),
    /// A page.
    Page(Box<PageFile>),
    /// Anything else, kept as its debug text.
    Other(std::string::String),
    /// A diagnostic.
    Diagnostic(Diagnostic),
}

impl Owned {
    fn from_item(item: Item<'_>) -> Self {
        match item {
            Item::Packet { packet, meta } => Self::Packet(packet, meta),
            Item::Page(page) => Self::Page(Box::new(*page)),
            Item::Diagnostic(diagnostic) => Self::Diagnostic(diagnostic),
            other => Self::Other(std::format!("{other:?}")),
        }
    }
}

/// Feeds `input` in pieces of `chunk` bytes and collects everything.
pub fn run(reader: &mut dyn Reader, input: &[u8], chunk: usize) -> Vec<Owned> {
    let mut items = Vec::new();
    for piece in input.chunks(chunk.max(1)) {
        reader.feed(piece, &mut |item| items.push(Owned::from_item(item)));
    }
    reader.finish(&mut |item| items.push(Owned::from_item(item)));
    items
}

/// Checks REQ-INP-001: one byte at a time, odd pieces and the whole input
/// give the same items. Returns them.
pub fn same_in_any_chunks<R: Reader>(make: impl Fn() -> R, input: &[u8]) -> Vec<Owned> {
    let whole = run(&mut make(), input, input.len());
    for chunk in [1, 2, 3, 7, 42, 64, 188, 1000] {
        assert_eq!(run(&mut make(), input, chunk), whole, "chunk size {chunk}");
    }
    whole
}

/// The packets among the items.
pub fn packets(items: &[Owned]) -> Vec<Packet> {
    items.iter().filter_map(|item| if let Owned::Packet(packet, _) = item { Some(*packet) } else { None }).collect()
}

/// The pages among the items.
pub fn pages(items: &[Owned]) -> Vec<PageFile> {
    items.iter().filter_map(|item| if let Owned::Page(page) = item { Some(**page) } else { None }).collect()
}

/// The diagnostics among the items.
pub fn diagnostics(items: &[Owned]) -> Vec<Diagnostic> {
    items
        .iter()
        .filter_map(|item| if let Owned::Diagnostic(diagnostic) = item { Some(*diagnostic) } else { None })
        .collect()
}
