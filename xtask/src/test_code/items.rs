//! The extents of the test-only items of a source: each item, field,
//! statement or expression whose outer attributes include a `cfg` that
//! holds only under `test`, read from the code that [`super::mask`] leaves.

use super::mask::is_identifier_byte;
use std::ops::Range;

/// Words that begin an item (the Reference, "Items"); an item ends at its
/// `;` or at the end of its first top-level block, never at a comma.
const ITEM_KEYWORDS: &[&str] = &[
    "mod",
    "fn",
    "impl",
    "struct",
    "enum",
    "trait",
    "const",
    "static",
    "type",
    "use",
    "extern",
    "macro_rules",
    "union",
];

/// The byte ranges of the test-only items of `code`, in order. An inner
/// `#![cfg(test)]` makes the rest of the module it is in test code: the
/// whole file at the top level.
pub(super) fn test_ranges(code: &[u8]) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut index = 0;
    while index < code.len() {
        if code[index] == b'#'
            && let Some(attribute) = attribute_at(code, index)
        {
            if is_test_cfg(&code[attribute.content.clone()]) {
                let end =
                    if attribute.inner { enclosing_end(code, attribute.end) } else { item_end(code, attribute.end) };
                ranges.push(index..end);
                index = end;
            } else {
                index = attribute.end;
            }
            continue;
        }
        index += 1;
    }
    ranges
}

/// One attribute: `#[…]` or `#![…]`.
struct Attribute {
    inner: bool,
    /// What is between the brackets.
    content: Range<usize>,
    /// Just after the closing bracket.
    end: usize,
}

/// The attribute whose `#` is at `index`, if one starts there.
fn attribute_at(code: &[u8], index: usize) -> Option<Attribute> {
    let mut open = skip_space(code, index + 1);
    let inner = code.get(open) == Some(&b'!');
    if inner {
        open = skip_space(code, open + 1);
    }
    if code.get(open) != Some(&b'[') {
        return None;
    }
    let close = matching_close(code, open);
    Some(Attribute { inner, content: open + 1..close, end: (close + 1).min(code.len()) })
}

/// Whether an attribute's content is a `cfg` whose predicate holds only
/// when `test` does: `test` itself, or an `all(…)` with such a member.
fn is_test_cfg(content: &[u8]) -> bool {
    let text: String =
        content.iter().filter(|byte| !byte.is_ascii_whitespace()).map(|&byte| char::from(byte)).collect();
    text.strip_prefix("cfg(").and_then(|rest| rest.strip_suffix(')')).is_some_and(implies_test)
}

fn implies_test(predicate: &str) -> bool {
    predicate == "test"
        || predicate
            .strip_prefix("all(")
            .and_then(|rest| rest.strip_suffix(')'))
            .is_some_and(|members| top_level_members(members).into_iter().any(implies_test))
}

/// The comma-separated members of a predicate list, nested lists kept whole.
fn top_level_members(list: &str) -> Vec<&str> {
    let mut members = Vec::new();
    let (mut depth, mut start) = (0usize, 0);
    for (index, byte) in list.bytes().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => {
                members.push(&list[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    members.push(&list[start..]);
    members
}

/// The end of the item, field, statement or expression that follows a
/// test-only attribute ending at `start`. Under-reaching is the safe error:
/// what is not blanked is still checked.
fn item_end(code: &[u8], start: usize) -> usize {
    let mut index = skip_space_and_attributes(code, start);
    if word_at(code, index) == "pub" {
        index = skip_space(code, index + "pub".len());
        if code.get(index) == Some(&b'(') {
            index = skip_space(code, matching_close(code, index) + 1);
        }
    }
    let item = ITEM_KEYWORDS.contains(&word_at(code, index)) || word_at(code, index) == "unsafe";
    if code.get(index) == Some(&b'{') {
        return (matching_close(code, index) + 1).min(code.len());
    }
    let mut depth = 0usize;
    let mut after_arrow = false;
    let mut position = index;
    while position < code.len() {
        match code[position] {
            // An item's body, or a match arm's block.
            b'{' if depth == 0 && (item || after_arrow) => return (matching_close(code, position) + 1).min(code.len()),
            b'(' | b'[' | b'{' => depth += 1,
            // The enclosing block, list or struct ends: so does the item.
            b')' | b']' | b'}' if depth == 0 => return position,
            b')' | b']' | b'}' => depth -= 1,
            b';' if depth == 0 => return position + 1,
            b',' if depth == 0 && !item => return position + 1,
            b'>' if depth == 0 && position > 0 && code[position - 1] == b'=' => after_arrow = true,
            _ => {}
        }
        position += 1;
    }
    code.len()
}

/// The position of the bracket that closes the block an inner attribute
/// ending at `start` is in, or the end of the code at the top level.
fn enclosing_end(code: &[u8], start: usize) -> usize {
    let mut depth = 0usize;
    for (offset, &byte) in code[start..].iter().enumerate() {
        match byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' if depth == 0 => return start + offset,
            b')' | b']' | b'}' => depth -= 1,
            _ => {}
        }
    }
    code.len()
}

/// The position of the bracket that closes the one at `open`, or the last
/// position when it is never closed.
fn matching_close(code: &[u8], open: usize) -> usize {
    let mut depth = 0usize;
    for (offset, &byte) in code[open..].iter().enumerate() {
        match byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return open + offset;
                }
            }
            _ => {}
        }
    }
    code.len().saturating_sub(1)
}

pub(super) fn skip_space(code: &[u8], mut index: usize) -> usize {
    while code.get(index).is_some_and(u8::is_ascii_whitespace) {
        index += 1;
    }
    index
}

fn skip_space_and_attributes(code: &[u8], start: usize) -> usize {
    let mut index = skip_space(code, start);
    while code.get(index) == Some(&b'#')
        && let Some(attribute) = attribute_at(code, index)
    {
        index = skip_space(code, attribute.end);
    }
    index
}

/// The identifier or keyword at `index`, empty if there is none.
pub(super) fn word_at(code: &[u8], index: usize) -> &str {
    let length = code.get(index..).map_or(0, |rest| rest.iter().take_while(|&&byte| is_identifier_byte(byte)).count());
    code.get(index..index + length).and_then(|word| std::str::from_utf8(word).ok()).unwrap_or_default()
}
