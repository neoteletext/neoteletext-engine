//! The paths a Rust source names, however they are written: the source
//! checks forbid some paths, and a check that looks for their text misses
//! `use std::{fs, io};`, `std :: fs` and a name given by `as`.

use crate::test_code::code_only;

/// A path a source names, with its segments joined by `::`, and the line it
/// is on. A `use` tree gives one for each leaf: `use a::{b, c::d};` names
/// `a::b` and `a::c::d`, and a glob's last segment is `*`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Named {
    pub(crate) line: usize,
    pub(crate) path: String,
    /// Written with `as`, so that what follows in the file names it by another
    /// word.
    pub(crate) renamed: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum Token {
    Word(String),
    Separator,
    Mark(u8),
}

/// The tokens of a source, each with its line, comments and literals left out.
fn tokens(source: &str) -> Vec<(usize, Token)> {
    let code = code_only(source);
    let mut found = Vec::new();
    let mut line = 1;
    let mut index = 0;
    while let Some(&byte) = code.get(index) {
        if byte == b'\n' {
            line += 1;
            index += 1;
        } else if byte.is_ascii_whitespace() {
            index += 1;
        } else if byte == b':' && code.get(index + 1) == Some(&b':') {
            found.push((line, Token::Separator));
            index += 2;
        } else if byte.is_ascii_alphanumeric() || byte == b'_' {
            let length = code[index..].iter().take_while(|byte| byte.is_ascii_alphanumeric() || **byte == b'_').count();
            let word = String::from_utf8_lossy(&code[index..index + length]).into_owned();
            found.push((line, Token::Word(word)));
            index += length;
        } else {
            found.push((line, Token::Mark(byte)));
            index += 1;
        }
    }
    found
}

/// Every path `source` names: its `use` trees expanded, `extern crate`, and
/// each other run of words joined by `::`, a single word among them.
pub(crate) fn named_paths(source: &str) -> Vec<Named> {
    let tokens = tokens(source);
    let mut named = Vec::new();
    let mut index = 0;
    while let Some((line, token)) = tokens.get(index) {
        match token {
            Token::Word(word) if word == "use" => index = use_tree(&tokens, index + 1, "", &mut named),
            Token::Word(word) if word == "extern" => index = extern_crate(&tokens, index, &mut named),
            Token::Word(_) | Token::Separator => {
                let (path, next) = plain_path(&tokens, index);
                named.push(Named { line: *line, path, renamed: false });
                index = next;
            }
            Token::Mark(_) => index += 1,
        }
    }
    named
}

/// `extern crate name as other;`, or any other use of the word `extern`.
fn extern_crate(tokens: &[(usize, Token)], index: usize, named: &mut Vec<Named>) -> usize {
    let word = |offset: usize| match tokens.get(index + offset) {
        Some((_, Token::Word(word))) => word.as_str(),
        _ => "",
    };
    if word(1) != "crate" {
        return index + 1;
    }
    let renamed = word(3) == "as";
    if let Some((line, _)) = tokens.get(index + 2) {
        named.push(Named { line: *line, path: word(2).to_string(), renamed });
    }
    index + if renamed { 5 } else { 3 }
}

/// The run of words joined by `::` at `index`, and the index after it.
fn plain_path(tokens: &[(usize, Token)], mut index: usize) -> (String, usize) {
    let mut path = String::new();
    while let Some((_, token)) = tokens.get(index) {
        match token {
            Token::Separator if path.is_empty() || path.ends_with("::") => (),
            Token::Separator => path.push_str("::"),
            Token::Word(word) if path.is_empty() || path.ends_with("::") => path.push_str(word),
            _ => break,
        }
        index += 1;
    }
    (path.trim_end_matches("::").to_string(), index)
}

fn joined(prefix: &str, segment: &str) -> String {
    if prefix.is_empty() { segment.to_string() } else { format!("{prefix}::{segment}") }
}

/// Reads one `use` tree from `index`, its paths under `prefix`, and gives the
/// index after it. `self` names the prefix itself.
fn use_tree(tokens: &[(usize, Token)], mut index: usize, prefix: &str, named: &mut Vec<Named>) -> usize {
    let mut path = prefix.to_string();
    let mut last_line = None;
    while let Some((line, token)) = tokens.get(index) {
        match token {
            Token::Separator => index += 1,
            Token::Word(word) if word == "as" => {
                named.push(Named { line: *line, path, renamed: true });
                return index + 2;
            }
            Token::Word(word) => {
                if word != "self" {
                    path = joined(&path, word);
                }
                last_line = Some(*line);
                index += 1;
            }
            Token::Mark(b'*') => {
                path = joined(&path, "*");
                last_line = Some(*line);
                index += 1;
            }
            Token::Mark(b'{') => {
                index += 1;
                while let Some((_, token)) = tokens.get(index) {
                    match token {
                        Token::Mark(b'}') => return index + 1,
                        Token::Mark(b',') => index += 1,
                        _ => index = use_tree(tokens, index, &path, named),
                    }
                }
                return index;
            }
            Token::Mark(_) => break,
        }
    }
    if let Some(line) = last_line {
        named.push(Named { line, path, renamed: false });
    }
    index
}

/// Whether `path` names `item`: is it, lies inside it, or ends in it.
pub(crate) fn names_item(path: &str, item: &str) -> bool {
    path == item
        || path.starts_with(&format!("{item}::"))
        || path.ends_with(&format!("::{item}"))
        || path.contains(&format!("::{item}::"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(source: &str) -> Vec<String> {
        named_paths(source).into_iter().map(|named| named.path).collect()
    }

    #[test]
    fn a_use_tree_gives_each_leaf() {
        assert_eq!(
            paths("use std::{fs, io::{self, Write}, net::*};"),
            ["std::fs", "std::io", "std::io::Write", "std::net::*"]
        );
    }

    #[test]
    fn a_path_is_read_across_spaces_and_lines() {
        assert_eq!(paths("let f = std :: fs ::\n    read;"), ["let", "f", "std::fs::read"]);
    }

    #[test]
    fn comments_and_literals_name_nothing() {
        assert_eq!(paths("// std::fs\n/* std::net */\nlet a = \"std::env\";"), ["let", "a"]);
    }

    #[test]
    fn a_rename_is_marked_with_its_line() {
        let named = named_paths("use std::thread\n    as worker;\nextern crate std as host;\nextern crate std;\n");
        let expected = [(2, "std::thread", true), (3, "std", true), (4, "std", false)];
        assert_eq!(named.iter().map(|n| (n.line, n.path.as_str(), n.renamed)).collect::<Vec<_>>(), expected.to_vec());
    }

    #[test]
    fn self_in_a_tree_can_be_renamed() {
        let named = named_paths("use std::{self as host};");
        assert_eq!(named, [Named { line: 1, path: "std".to_string(), renamed: true }]);
    }

    #[test]
    fn a_precise_capture_is_no_use_tree() {
        assert!(paths("fn f() -> impl Sized + use<> {}").iter().all(|path| !path.contains("::")));
    }

    #[test]
    fn an_item_is_named_whole() {
        assert!(names_item("std::fs::read", "std::fs"));
        assert!(names_item("std::time::SystemTime", "SystemTime"));
        assert!(names_item("SystemTime::now", "SystemTime"));
        assert!(!names_item("std::fsx", "std::fs"));
        assert!(!names_item("MySystemTime", "SystemTime"));
    }
}
