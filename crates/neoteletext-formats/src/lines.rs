//! The bounded line reader the TTI and edit.tf readers share: input fed in
//! pieces of any size is cut at each line feed into lines of at most `N`
//! bytes, each with its number and the byte offset of its first byte, so
//! that every piece size gives the same lines (REQ-INP-001). A longer line
//! keeps its first `N` bytes and is marked as overflowing; memory is fixed
//! by `N`.

use crate::item::offset_of;

/// A line as read, without its line feed. Its number and offset are
/// [`Lines::number`] and [`Lines::offset`], which the reader's handling of
/// the line reports diagnostics with.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Line<'a> {
    /// The line's bytes, at most `N` of them.
    pub(crate) text: &'a [u8],
    /// True when the line was longer than `N` bytes and cut.
    pub(crate) overflow: bool,
}

/// Lines of at most `N` bytes, read from input fed in pieces.
///
/// The reader that holds one calls [`Lines::take`] until it returns `None`
/// for each piece, and [`Lines::finish`] at the end of input; after each
/// line it handles [`Lines::line`] and then calls [`Lines::clear`]. It
/// does not take a callback, because the reader's handling of a line
/// borrows the reader that holds this.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Lines<const N: usize> {
    bytes: [u8; N],
    len: usize,
    overflow: bool,
    number: u32,
    offset: u64,
    position: u64,
}

impl<const N: usize> Lines<N> {
    /// No line read yet.
    pub(crate) const fn new() -> Self {
        Self { bytes: [0; N], len: 0, overflow: false, number: 0, offset: 0, position: 0 }
    }

    /// Takes bytes of `bytes` into the line until a line feed ends it, and
    /// returns the bytes after that line feed; `None` when every byte was
    /// taken and no line ended. A line ended is [`Lines::line`] until
    /// [`Lines::clear`], which must come before the next `take`.
    pub(crate) fn take<'a>(&mut self, bytes: &'a [u8]) -> Option<&'a [u8]> {
        let base = self.position;
        for (index, &byte) in bytes.iter().enumerate() {
            if self.len == 0 && !self.overflow {
                self.offset = offset_of(base, index);
            }
            if byte == b'\n' {
                let after = index.saturating_add(1);
                self.position = offset_of(base, after);
                self.number = self.number.saturating_add(1);
                return Some(bytes.get(after..).unwrap_or(&[]));
            }
            if let Some(slot) = self.bytes.get_mut(self.len) {
                *slot = byte;
                self.len = self.len.saturating_add(1);
            } else {
                self.overflow = true;
            }
        }
        self.position = offset_of(base, bytes.len());
        None
    }

    /// Ends the last line at the end of input when it has no line feed.
    /// Returns true when there is such a line, which is then
    /// [`Lines::line`] until [`Lines::clear`].
    pub(crate) fn finish(&mut self) -> bool {
        let pending = self.len > 0 || self.overflow;
        if pending {
            self.number = self.number.saturating_add(1);
        }
        pending
    }

    /// The line ended last.
    pub(crate) fn line(&self) -> Line<'_> {
        Line { text: self.bytes.get(..self.len).unwrap_or(&[]), overflow: self.overflow }
    }

    /// The number of the line ended last, from 1; 0 before the first.
    pub(crate) const fn number(&self) -> u32 {
        self.number
    }

    /// The stream offset of the first byte of the line ended last.
    pub(crate) const fn offset(&self) -> u64 {
        self.offset
    }

    /// Empties the line once it has been handled.
    pub(crate) const fn clear(&mut self) {
        self.len = 0;
        self.overflow = false;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;
    use std::vec::Vec;

    /// Every line of `input` fed in pieces of `piece` bytes, then finished.
    fn lines<const N: usize>(input: &[u8], piece: usize) -> Vec<(Vec<u8>, u32, u64, bool)> {
        let mut reader = Lines::<N>::new();
        let mut found = Vec::new();
        let mut record = |reader: &mut Lines<N>| {
            let line = reader.line();
            found.push((line.text.to_vec(), reader.number(), reader.offset(), line.overflow));
            reader.clear();
        };
        for chunk in input.chunks(piece) {
            let mut rest = chunk;
            while let Some(after) = reader.take(rest) {
                record(&mut reader);
                rest = after;
            }
        }
        if reader.finish() {
            record(&mut reader);
        }
        found
    }

    #[test]
    fn lines_are_numbered_bounded_and_the_same_in_any_pieces() {
        let input = b"ab\n\nlonger\nend";
        let expected = std::vec![
            (b"ab".to_vec(), 1, 0, false),
            (Vec::new(), 2, 3, false),
            (b"lon".to_vec(), 3, 4, true),
            (b"end".to_vec(), 4, 11, false),
        ];
        for piece in 1..=input.len() {
            assert_eq!(lines::<3>(input, piece), expected, "pieces of {piece}");
        }
        assert_eq!(lines::<3>(b"", 1), []);
        assert_eq!(lines::<3>(b"\n", 1), [(Vec::new(), 1, 0, false)]);
    }
}
