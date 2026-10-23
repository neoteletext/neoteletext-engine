//! The Foundation's fixed-capacity collection (REQ-GEN-002).

/// A vector with fixed capacity, held inline: the Foundation's
/// fixed-capacity collection (no allocation, REQ-GEN-002). Equality
/// compares the items only, not the unused slots.
#[derive(Clone, Copy, Debug)]
pub struct FixedVec<T: Copy, const N: usize> {
    items: [T; N],
    len: usize,
}

impl<T: Copy + PartialEq, const N: usize> PartialEq for FixedVec<T, N> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: Copy + Eq, const N: usize> Eq for FixedVec<T, N> {}

impl<T: Copy, const N: usize> FixedVec<T, N> {
    /// An empty vector whose unused slots hold `filler`.
    #[must_use]
    pub const fn new(filler: T) -> Self {
        Self { items: [filler; N], len: 0 }
    }

    /// Appends an item. Returns false, changing nothing, when full.
    pub fn push(&mut self, item: T) -> bool {
        match self.items.get_mut(self.len) {
            Some(slot) => {
                *slot = item;
                self.len = self.len.saturating_add(1);
                true
            }
            None => false,
        }
    }

    /// Removes and returns the last item.
    pub fn pop(&mut self) -> Option<T> {
        let last = self.len.checked_sub(1)?;
        self.len = last;
        self.items.get(last).copied()
    }

    /// Removes the item at `index`, shifting later items down.
    pub fn remove(&mut self, index: usize) -> Option<T> {
        let removed = self.as_slice().get(index).copied()?;
        let len = self.len;
        // A rotation of an empty slice panics (REQ-GEN-006).
        if let Some(tail) = self.items.get_mut(index..len).filter(|tail| !tail.is_empty()) {
            tail.rotate_left(1);
        }
        self.len = len.saturating_sub(1);
        Some(removed)
    }

    /// Empties the vector.
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Number of items.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// True when empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// True when no more items fit.
    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.len >= N
    }

    /// The items as a slice.
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        self.items.get(..self.len).unwrap_or(&[])
    }

    /// The items as a mutable slice.
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        let len = self.len;
        self.items.get_mut(..len).unwrap_or(&mut [])
    }

    /// Shortens the vector to at most `len` items.
    pub fn truncate(&mut self, len: usize) {
        self.len = self.len.min(len);
    }
}
