//! A byte string with a fixed capacity, held inline, for the text fields of
//! page files (descriptions, link labels and addresses). No allocation.

/// Up to `N` bytes of text. Bytes are kept as they appeared in the file;
/// callers decide which characters are acceptable.
#[derive(Clone, Copy, Debug)]
pub struct Text<const N: usize> {
    bytes: [u8; N],
    len: usize,
}

// Equality and hashing look only at the bytes held, not at storage past
// the end that an earlier, longer value left behind.
impl<const N: usize> PartialEq for Text<N> {
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl<const N: usize> Eq for Text<N> {}

impl<const N: usize> core::hash::Hash for Text<N> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.as_bytes().hash(state);
    }
}

impl<const N: usize> Default for Text<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Text<N> {
    /// The empty text.
    #[must_use]
    pub const fn new() -> Self {
        Self { bytes: [0; N], len: 0 }
    }

    /// Text holding as much of `bytes` as fits. The flag is true when some
    /// bytes did not fit.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> (Self, bool) {
        let mut text = Self::new();
        let truncated = !text.set(bytes);
        (text, truncated)
    }

    /// Replaces the contents with as much of `bytes` as fits. Returns false
    /// when some bytes did not fit.
    pub fn set(&mut self, bytes: &[u8]) -> bool {
        self.len = copy_prefix(&mut self.bytes, bytes);
        self.len == bytes.len()
    }

    /// Appends one byte. Returns false, changing nothing, when full.
    pub fn push(&mut self, byte: u8) -> bool {
        match self.bytes.get_mut(self.len) {
            Some(slot) => {
                *slot = byte;
                self.len = self.len.saturating_add(1);
                true
            }
            None => false,
        }
    }

    /// The bytes held.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.get(..self.len).unwrap_or(&[])
    }

    /// Number of bytes held.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// True when empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Empties the text.
    pub fn clear(&mut self) {
        self.len = 0;
    }
}

/// Copies as many bytes as both slices hold from the start of `source` to
/// the start of `target`, and returns how many.
///
/// REQ-GEN-019: a loop over the pair, not `copy_from_slice`. Where the
/// optimiser cannot see that two slices cut to the same length are equal,
/// `copy_from_slice` links its length-mismatch panic into the image.
pub(crate) fn copy_prefix(target: &mut [u8], source: &[u8]) -> usize {
    for (to, from) in target.iter_mut().zip(source) {
        *to = *from;
    }
    target.len().min(source.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_prefix_copies_the_shorter_length() {
        let mut target = [0u8; 3];
        assert_eq!(copy_prefix(&mut target, b"ab"), 2);
        assert_eq!(target, *b"ab\0");
        assert_eq!(copy_prefix(&mut target, b"wxyz"), 3);
        assert_eq!(target, *b"wxy");
        assert_eq!(copy_prefix(&mut [], b"a"), 0);
    }

    #[test]
    fn truncates_and_reports() {
        let (text, truncated) = Text::<4>::from_bytes(b"abcdef");
        assert!(truncated);
        assert_eq!(text.as_bytes(), b"abcd");
        let (text, truncated) = Text::<4>::from_bytes(b"ab");
        assert!(!truncated);
        assert_eq!(text.as_bytes(), b"ab");
        let mut text = Text::<2>::new();
        assert!(text.push(b'x') && text.push(b'y') && !text.push(b'z'));
        assert_eq!(text.len(), 2);
    }
}
