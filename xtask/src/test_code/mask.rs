//! A Rust source's code with its comments and literals blanked, so that its
//! structure (brackets, attributes, `mod` lines) can be read without text
//! in a comment or a string being taken for code.

/// `text` with every byte of a comment, a string literal or a character
/// literal replaced by a space, newlines kept. The result has the length of
/// `text`, so a position in it is a position in `text`.
pub(super) fn code_only(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut code = bytes.to_vec();
    let mut index = 0;
    while index < bytes.len() {
        match literal_end(bytes, index) {
            Some(end) => {
                let end = end.min(bytes.len());
                for byte in &mut code[index..end] {
                    if *byte != b'\n' {
                        *byte = b' ';
                    }
                }
                index = end;
            }
            None => index += 1,
        }
    }
    code
}

/// The end (exclusive) of the comment or literal that starts at `index`, or
/// `None` when code starts there.
fn literal_end(bytes: &[u8], index: usize) -> Option<usize> {
    let at = |offset: usize| bytes.get(index + offset).copied();
    match at(0)? {
        b'/' if at(1) == Some(b'/') => {
            Some(bytes[index..].iter().position(|&byte| byte == b'\n').map_or(bytes.len(), |line| index + line))
        }
        b'/' if at(1) == Some(b'*') => Some(block_comment_end(bytes, index)),
        b'"' => Some(string_end(bytes, index + 1)),
        b'\'' => char_end(bytes, index),
        b'r' | b'b' | b'c' if !follows_identifier(bytes, index) => prefixed_literal_end(bytes, index),
        _ => None,
    }
}

/// Whether the byte before `index` belongs to an identifier, so that an `r`,
/// `b` or `c` there is part of a name, not a literal's prefix.
fn follows_identifier(bytes: &[u8], index: usize) -> bool {
    index.checked_sub(1).and_then(|before| bytes.get(before)).is_some_and(|&byte| is_identifier_byte(byte))
}

/// Whether `byte` may be part of an identifier (ASCII; a non-ASCII name is
/// treated as code, which only makes the blanking smaller).
pub(super) fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// A block comment's end; block comments nest (the Reference, "Comments").
fn block_comment_end(bytes: &[u8], start: usize) -> usize {
    let mut depth = 0usize;
    let mut index = start;
    while index + 1 < bytes.len() {
        match (bytes[index], bytes[index + 1]) {
            (b'/', b'*') => {
                depth += 1;
                index += 2;
            }
            (b'*', b'/') => {
                depth -= 1;
                index += 2;
                if depth == 0 {
                    return index;
                }
            }
            _ => index += 1,
        }
    }
    bytes.len()
}

/// The end of a quoted string whose contents start at `index`, escapes
/// skipped.
fn string_end(bytes: &[u8], mut index: usize) -> usize {
    while let Some(&byte) = bytes.get(index) {
        match byte {
            b'\\' => index += 2,
            b'"' => return index + 1,
            _ => index += 1,
        }
    }
    bytes.len()
}

/// A character literal's end, or `None` for a lifetime or a label, which
/// start with a quote too but have no closing one after their first
/// character.
fn char_end(bytes: &[u8], quote: usize) -> Option<usize> {
    let first = *bytes.get(quote + 1)?;
    if first == b'\\' {
        let close = bytes.get(quote + 3..)?.iter().position(|&byte| byte == b'\'')?;
        return Some(quote + 3 + close + 1);
    }
    // The length of the UTF-8 sequence that `first` leads.
    let width = match first {
        0xF0.. => 4,
        0xE0.. => 3,
        0xC0.. => 2,
        _ => 1,
    };
    (bytes.get(quote + 1 + width) == Some(&b'\'')).then_some(quote + 2 + width)
}

/// The end of a raw string (`r"…"`, `r#"…"#`), a byte or C string (`b"…"`,
/// `br"…"`, `c"…"`) or a byte character (`b'…'`) starting at `index`.
fn prefixed_literal_end(bytes: &[u8], index: usize) -> Option<usize> {
    let at = |offset: usize| bytes.get(index + offset).copied();
    let raw_at = match (at(0)?, at(1)) {
        (b'b' | b'c', Some(b'"')) => return Some(string_end(bytes, index + 2)),
        (b'b', Some(b'\'')) => return char_end(bytes, index + 1),
        (b'b' | b'c', Some(b'r')) => index + 1,
        (b'r', _) => index,
        _ => return None,
    };
    let hashes = bytes[raw_at + 1..].iter().take_while(|&&byte| byte == b'#').count();
    let open = raw_at + 1 + hashes;
    if bytes.get(open) != Some(&b'"') {
        return None;
    }
    let mut index = open + 1;
    while index < bytes.len() {
        if bytes[index] == b'"'
            && bytes[index + 1..].iter().take(hashes).filter(|&&byte| byte == b'#').count() == hashes
        {
            return Some(index + 1 + hashes);
        }
        index += 1;
    }
    Some(bytes.len())
}
