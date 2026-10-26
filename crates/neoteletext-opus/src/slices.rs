//! Slice operations that cannot panic, whatever the optimiser does.
//!
//! `copy_from_slice`, `copy_within` and `step_by` check their lengths and step and panic when
//! the check fails. A build optimised for size (`opt-level = "z"`, as the engine's microcontroller
//! image is built, REQ-GEN-019) calls them out of line, so the checks stay linked even where they
//! can never fail. These forms have no check to fail: they do what the standard forms do on every
//! length the decoder passes, and on any other length they stop at the shorter slice.

/// Copy `source` into the front of `target`, as many values as both hold.
#[inline]
pub(crate) fn copy<T: Copy>(target: &mut [T], source: &[T]) {
    for (to, &from) in target.iter_mut().zip(source) {
        *to = from;
    }
}

/// Drop the first `count` values of `buffer` by moving the rest to its front; what is left at the
/// end keeps its old values (`buffer.copy_within(count.., 0)`).
#[inline]
pub(crate) fn discard_front<T: Copy>(buffer: &mut [T], count: usize) {
    let mut from = count;
    let mut to = 0;
    while let Some(&value) = buffer.get(from) {
        if let Some(slot) = buffer.get_mut(to) {
            *slot = value;
        }
        from = from.saturating_add(1);
        to = to.saturating_add(1);
    }
}

/// Move the first `count` values of `buffer` up by `by` places, last first so that they overlap
/// safely (`buffer.copy_within(..count, by)`); values that would pass the end are dropped.
#[inline]
pub(crate) fn move_up<T: Copy>(buffer: &mut [T], count: usize, by: usize) {
    for from in (0..count).rev() {
        if let (Some(&value), Some(to)) = (buffer.get(from), from.checked_add(by))
            && let Some(slot) = buffer.get_mut(to)
        {
            *slot = value;
        }
    }
}

/// Every `step`th value of `slice` from `start`: `slice.iter().skip(start).step_by(step)`, with a
/// step of 0 taken as 1.
#[inline]
pub(crate) fn strided<T>(slice: &[T], start: usize, step: usize) -> impl Iterator<Item = &T> + Clone {
    let step = step.max(1);
    let mut at = start;
    core::iter::from_fn(move || {
        let value = slice.get(at)?;
        at = at.saturating_add(step);
        Some(value)
    })
}

/// Every `step`th value of `slice` downwards from `start`, down to the front: what
/// `iter().step_by(step)` gives after `rev()` on a slice whose last value is at `start`, with a step of
/// 0 taken as 1; nothing if `start` is past the end.
#[inline]
pub(crate) fn strided_down<T>(slice: &[T], start: usize, step: usize) -> impl Iterator<Item = &T> {
    let step = step.max(1);
    let mut at = Some(start);
    core::iter::from_fn(move || {
        let here = at?;
        let value = slice.get(here)?;
        at = here.checked_sub(step);
        Some(value)
    })
}

/// Apply `f` to every `step`th value of `slice` from `start`, as `strided` reads them.
#[inline]
pub(crate) fn for_each_strided<T>(slice: &mut [T], start: usize, step: usize, mut f: impl FnMut(&mut T)) {
    let step = step.max(1);
    let mut at = start;
    while let Some(value) = slice.get_mut(at) {
        f(value);
        at = at.saturating_add(step);
    }
}

/// The whole chunks of `size` values of `slice`, in order (`slice.chunks_exact(size)`); none for
/// a size of 0.
#[inline]
pub(crate) fn chunks<T>(slice: &[T], size: usize) -> impl Iterator<Item = &[T]> + Clone {
    let mut rest = slice;
    core::iter::from_fn(move || {
        let (chunk, after) = rest.split_at_checked(size).filter(|_| size > 0)?;
        rest = after;
        Some(chunk)
    })
}

/// `chunks`, mutable (`slice.chunks_exact_mut(size)`).
#[inline]
pub(crate) fn chunks_mut<T>(slice: &mut [T], size: usize) -> impl Iterator<Item = &mut [T]> {
    let mut rest = slice;
    core::iter::from_fn(move || {
        if size == 0 {
            return None;
        }
        let (chunk, after) = core::mem::take(&mut rest).split_at_mut_checked(size)?;
        rest = after;
        Some(chunk)
    })
}

/// The chunks of at most `size` values of `slice`, the last perhaps shorter (`slice.chunks(size)`);
/// none for a size of 0.
#[inline]
pub(crate) fn chunks_up_to<T>(slice: &[T], size: usize) -> impl Iterator<Item = &[T]> {
    let mut rest = slice;
    core::iter::from_fn(move || {
        if size == 0 || rest.is_empty() {
            return None;
        }
        let (chunk, after) = rest.split_at_checked(size).unwrap_or((rest, &[]));
        rest = after;
        Some(chunk)
    })
}

/// `chunks_up_to`, mutable (`slice.chunks_mut(size)`).
#[inline]
pub(crate) fn chunks_up_to_mut<T>(slice: &mut [T], size: usize) -> impl Iterator<Item = &mut [T]> {
    let mut rest = slice;
    core::iter::from_fn(move || {
        if size == 0 || rest.is_empty() {
            return None;
        }
        let taken = core::mem::take(&mut rest);
        let at = size.min(taken.len());
        let (chunk, after) = taken.split_at_mut_checked(at)?;
        rest = after;
        Some(chunk)
    })
}

/// The `index`th whole chunk of `size` values (`slice.chunks_exact(size).nth(index)`).
#[inline]
pub(crate) fn nth_chunk<T>(slice: &[T], size: usize, index: usize) -> Option<&[T]> {
    slice.get(index.checked_mul(size)?..)?.get(..size).filter(|_| size > 0)
}

/// `nth_chunk`, mutable.
#[inline]
pub(crate) fn nth_chunk_mut<T>(slice: &mut [T], size: usize, index: usize) -> Option<&mut [T]> {
    slice.get_mut(index.checked_mul(size)?..)?.get_mut(..size).filter(|_| size > 0)
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, reason = "tests may panic")]
mod tests {
    use super::{
        chunks, chunks_mut, chunks_up_to, chunks_up_to_mut, copy, discard_front, for_each_strided, move_up, nth_chunk,
        strided,
    };

    #[test]
    fn the_chunks_are_the_standard_forms() {
        // Sizes the compiler cannot see, as the decoder's are.
        let (three, four) = (core::hint::black_box(3), core::hint::black_box(4));
        let values: [i32; 10] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        assert!(chunks(&values, three).eq(values.chunks_exact(three)));
        assert!(chunks_up_to(&values, three).eq(values.chunks(three)));
        assert_eq!(nth_chunk(&values, three, 2), values.chunks_exact(three).nth(2));
        assert_eq!(nth_chunk(&values, three, 3), None);
        let (mut a, mut b) = (values, values);
        chunks_mut(&mut a, four).for_each(<[i32]>::reverse);
        b.chunks_exact_mut(four).for_each(<[i32]>::reverse);
        assert_eq!(a, b);
        chunks_up_to_mut(&mut a, four).for_each(|c| c[0] = -1);
        b.chunks_mut(four).for_each(|c| c[0] = -1);
        assert_eq!(a, b);
        assert_eq!(chunks(&values, 0).count() + chunks_up_to(&values, 0).count(), 0);
    }

    #[test]
    fn each_does_what_the_standard_form_does() {
        let mut a = [1, 2, 3, 4, 5];
        let mut b = a;
        discard_front(&mut a, 2);
        b.copy_within(2.., 0);
        assert_eq!(a, b);
        let (mut a, mut b) = ([1, 2, 3, 4, 5, 6], [1, 2, 3, 4, 5, 6]);
        move_up(&mut a, 3, 2);
        b.copy_within(..3, 2);
        assert_eq!(a, b);
        let mut target = [0; 3];
        copy(&mut target, &[7, 8, 9]);
        assert_eq!(target, [7, 8, 9]);
        let values: [i32; 10] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        assert!(strided(&values, 1, 3).copied().eq(values.iter().skip(1).step_by(3).copied()));
        let mut marked = [0; 10];
        for_each_strided(&mut marked, 2, 4, |v| *v = 1);
        assert_eq!(marked, [0, 0, 1, 0, 0, 0, 1, 0, 0, 0]);
    }

    #[test]
    fn lengths_beyond_the_slice_stop_at_its_end() {
        let mut a = [1, 2, 3];
        discard_front(&mut a, 5);
        assert_eq!(a, [1, 2, 3]);
        move_up(&mut a, 3, 5);
        assert_eq!(a, [1, 2, 3]);
        assert_eq!(strided(&a, 0, 0).count(), 3);
    }
}
