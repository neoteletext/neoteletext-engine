//! The session instructions a registration or refresh answers with
//! (section 9.6): the JSON read into a bounded value, unknown members
//! stepped over.

use super::{
    MAX_CREDENTIAL_NAME, MAX_CREDENTIALS, MAX_INSTRUCTIONS_BYTES, MAX_RULE_TEXT, MAX_SCOPE_RULES, MAX_SESSION_ID,
    MAX_SESSION_URL,
};
use crate::bytes::{Overflow, TextBuf};
use crate::json::{JsonReader, JsonStr, Token};

/// One rule of a session's `scope_specification`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ScopeRule {
    /// `include` (true) or `exclude`.
    pub include: bool,
    /// The host pattern: `*`, `*.example.com` or a host.
    pub domain: TextBuf<MAX_RULE_TEXT>,
    /// The path prefix.
    pub path: TextBuf<MAX_RULE_TEXT>,
}

/// What a registration or refresh answered (section 9.6).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Instructions {
    /// The session's identifier.
    pub id: TextBuf<MAX_SESSION_ID>,
    /// The refresh address as given, when given; else the request's own.
    pub refresh_url: Option<TextBuf<MAX_SESSION_URL>>,
    /// The scope's origin as given, when given.
    pub origin: Option<TextBuf<MAX_SESSION_URL>>,
    /// Site scope (true) or origin scope.
    pub include_site: bool,
    /// The scope rules, in the order given.
    pub rules: [Option<ScopeRule>; MAX_SCOPE_RULES],
    /// The names of the cookies the session keeps.
    pub credentials: [Option<TextBuf<MAX_CREDENTIAL_NAME>>; MAX_CREDENTIALS],
}

/// The instructions' verdict.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "no allocation: the instructions are read once into a value the caller copies into the session store"
)]
pub enum Answer {
    /// The session continues, as these instructions say.
    Continue(Instructions),
    /// `"continue": false`: the session ends.
    End,
}

/// Why instructions were refused; the session then ends (section 8.9).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InstructionsError {
    /// Not JSON, or longer than [`MAX_INSTRUCTIONS_BYTES`].
    Malformed,
    /// A required member is missing or of the wrong type.
    Missing,
    /// A value is longer than its bound, or there are more rules or
    /// credentials than are kept.
    TooLarge,
    /// A credential is not a cookie, has no name, or is `Partitioned`.
    Credential,
}

type Read<T> = Result<T, InstructionsError>;

/// The members each object of the instructions may name once (section
/// 9.6).
const INSTRUCTION_MEMBERS: [&[u8]; 5] = [b"session_identifier", b"refresh_url", b"continue", b"scope", b"credentials"];
const SCOPE_MEMBERS: [&[u8]; 3] = [b"origin", b"include_site", b"scope_specification"];
const RULE_MEMBERS: [&[u8]; 3] = [b"type", b"domain", b"path"];
const CREDENTIAL_MEMBERS: [&[u8]; 3] = [b"type", b"name", b"attributes"];

fn next<'a>(reader: &mut JsonReader<'a>) -> Read<Token<'a>> {
    reader.next_token().map_err(|_| InstructionsError::Malformed)?.ok_or(InstructionsError::Malformed)
}

fn text<const N: usize>(value: JsonStr<'_>) -> Read<TextBuf<N>> {
    let mut bytes = [0_u8; N];
    let length = value.decode_into(&mut bytes).map_err(|Overflow| InstructionsError::TooLarge)?;
    TextBuf::from_bytes(bytes.get(..length).unwrap_or(&[])).ok_or(InstructionsError::TooLarge)
}

fn string<const N: usize>(reader: &mut JsonReader<'_>) -> Read<TextBuf<N>> {
    match next(reader)? {
        Token::Str(value) => text(value),
        _ => Err(InstructionsError::Missing),
    }
}

/// Reads a session identifier, which the agent writes back into
/// `Sec-Secure-Session-Id`: refused unless it is non-empty and every byte
/// is visible ASCII, `0x21..=0x7E` (RFC 9110 section 5.6.2's `VCHAR`), so
/// it can never carry a line break, a control byte or a space into a
/// header (finding F-14, decision E.114 item 6).
fn identifier(reader: &mut JsonReader<'_>) -> Read<TextBuf<MAX_SESSION_ID>> {
    let id: TextBuf<MAX_SESSION_ID> = string(reader)?;
    let bytes = id.as_bytes();
    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_graphic) {
        return Err(InstructionsError::Malformed);
    }
    Ok(id)
}

fn boolean(reader: &mut JsonReader<'_>) -> Read<bool> {
    match next(reader)? {
        Token::Bool(value) => Ok(value),
        _ => Err(InstructionsError::Missing),
    }
}

/// Steps over the value whose first token is next.
fn skip(reader: &mut JsonReader<'_>) -> Read<()> {
    let first = next(reader)?;
    reader.skip(first).map_err(|_| InstructionsError::Malformed)
}

/// Calls `member` with each member name of the object `first` begins;
/// `member` reads the value or returns false to have it skipped. A member
/// of `known` named twice is refused, so a later one cannot replace an
/// earlier (decision E.114 item 6, finding F-92).
fn object<'a, const KNOWN: usize>(
    first: Token<'a>,
    reader: &mut JsonReader<'a>,
    known: [&[u8]; KNOWN],
    mut member: impl FnMut(JsonStr<'a>, &mut JsonReader<'a>) -> Read<bool>,
) -> Read<()> {
    if first != Token::BeginObject {
        return Err(InstructionsError::Missing);
    }
    let mut seen = [false; KNOWN];
    loop {
        match next(reader)? {
            Token::EndObject => return Ok(()),
            Token::Key(name) => {
                if let Some(index) = known.iter().position(|known| name.equals(known))
                    && let Some(seen) = seen.get_mut(index)
                {
                    if *seen {
                        return Err(InstructionsError::Malformed);
                    }
                    *seen = true;
                }
                if !member(name, reader)? {
                    skip(reader)?;
                }
            }
            _ => return Err(InstructionsError::Malformed),
        }
    }
}

/// Calls `element` with the first token of each element of the array
/// `first` begins.
fn array<'a>(
    first: Token<'a>,
    reader: &mut JsonReader<'a>,
    mut element: impl FnMut(Token<'a>, &mut JsonReader<'a>) -> Read<()>,
) -> Read<()> {
    if first != Token::BeginArray {
        return Err(InstructionsError::Missing);
    }
    loop {
        match next(reader)? {
            Token::EndArray => return Ok(()),
            token => element(token, reader)?,
        }
    }
}

/// Reads session instructions (section 9.6). Unknown members are stepped
/// over, as the specification asks of a client meeting newer ones.
///
/// # Errors
/// Anything [`InstructionsError`] names.
pub fn read_instructions(body: &[u8]) -> Read<Answer> {
    if body.len() > MAX_INSTRUCTIONS_BYTES {
        return Err(InstructionsError::Malformed);
    }
    let mut reader = JsonReader::new(body).map_err(|_| InstructionsError::Malformed)?;
    let mut id = None;
    let mut refresh_url = None;
    let mut proceed = true;
    let mut scope = None;
    let mut credentials = None;
    let first = next(&mut reader)?;
    object(first, &mut reader, INSTRUCTION_MEMBERS, |name, reader| {
        if name.equals(b"session_identifier") {
            id = Some(identifier(reader)?);
        } else if name.equals(b"refresh_url") {
            refresh_url = Some(string(reader)?);
        } else if name.equals(b"continue") {
            proceed = boolean(reader)?;
        } else if name.equals(b"scope") {
            let first = next(reader)?;
            scope = Some(read_scope(first, reader)?);
        } else if name.equals(b"credentials") {
            let first = next(reader)?;
            credentials = Some(read_credentials(first, reader)?);
        } else {
            return Ok(false);
        }
        Ok(true)
    })?;
    if reader.next_token() != Ok(None) {
        return Err(InstructionsError::Malformed);
    }
    if !proceed {
        return Ok(Answer::End);
    }
    let (origin, include_site, rules) = scope.ok_or(InstructionsError::Missing)?;
    let credentials = credentials.ok_or(InstructionsError::Missing)?;
    if credentials.iter().all(Option::is_none) {
        return Err(InstructionsError::Missing);
    }
    Ok(Answer::Continue(Instructions {
        id: id.ok_or(InstructionsError::Missing)?,
        refresh_url,
        origin,
        include_site: include_site.ok_or(InstructionsError::Missing)?,
        rules,
        credentials,
    }))
}

type Scope = (Option<TextBuf<MAX_SESSION_URL>>, Option<bool>, [Option<ScopeRule>; MAX_SCOPE_RULES]);

fn read_scope<'a>(first: Token<'a>, reader: &mut JsonReader<'a>) -> Read<Scope> {
    let mut origin = None;
    let mut include_site = None;
    let mut rules = [None; MAX_SCOPE_RULES];
    object(first, reader, SCOPE_MEMBERS, |name, reader| {
        if name.equals(b"origin") {
            origin = Some(string(reader)?);
        } else if name.equals(b"include_site") {
            include_site = Some(boolean(reader)?);
        } else if name.equals(b"scope_specification") {
            let mut count = 0_usize;
            let first = next(reader)?;
            array(first, reader, |first, reader| {
                let rule = read_rule(first, reader)?;
                let slot = rules.get_mut(count).ok_or(InstructionsError::TooLarge)?;
                *slot = Some(rule);
                count = count.saturating_add(1);
                Ok(())
            })?;
        } else {
            return Ok(false);
        }
        Ok(true)
    })?;
    Ok((origin, include_site, rules))
}

fn read_rule<'a>(first: Token<'a>, reader: &mut JsonReader<'a>) -> Read<ScopeRule> {
    let mut include = None;
    let mut domain = TextBuf::from_bytes(b"*").ok_or(InstructionsError::TooLarge)?;
    let mut path = TextBuf::from_bytes(b"/").ok_or(InstructionsError::TooLarge)?;
    object(first, reader, RULE_MEMBERS, |name, reader| {
        if name.equals(b"type") {
            let kind: TextBuf<MAX_RULE_TEXT> = string(reader)?;
            include = match kind.as_bytes() {
                b"include" => Some(true),
                b"exclude" => Some(false),
                _ => return Err(InstructionsError::Missing),
            };
        } else if name.equals(b"domain") {
            domain = string(reader)?;
        } else if name.equals(b"path") {
            path = string(reader)?;
        } else {
            return Ok(false);
        }
        Ok(true)
    })?;
    Ok(ScopeRule { include: include.ok_or(InstructionsError::Missing)?, domain, path })
}

type Credentials = [Option<TextBuf<MAX_CREDENTIAL_NAME>>; MAX_CREDENTIALS];

fn read_credentials<'a>(first: Token<'a>, reader: &mut JsonReader<'a>) -> Read<Credentials> {
    let mut credentials = [None; MAX_CREDENTIALS];
    let mut count = 0_usize;
    array(first, reader, |first, reader| {
        let mut cookie = false;
        let mut name: Option<TextBuf<MAX_CREDENTIAL_NAME>> = None;
        let mut partitioned = false;
        object(first, reader, CREDENTIAL_MEMBERS, |member, reader| {
            if member.equals(b"type") {
                let kind: TextBuf<MAX_RULE_TEXT> = string(reader)?;
                cookie = kind.as_bytes() == b"cookie";
            } else if member.equals(b"name") {
                name = Some(string(reader)?);
            } else if member.equals(b"attributes") {
                let attributes: TextBuf<MAX_SESSION_URL> = string(reader)?;
                // Section 9.6: a partitioned cookie cannot be bound.
                partitioned = attributes
                    .as_bytes()
                    .split(|&byte| byte == b';')
                    .any(|attribute| crate::bytes::trim_ows(attribute).eq_ignore_ascii_case(b"Partitioned"));
            } else {
                return Ok(false);
            }
            Ok(true)
        })?;
        let name = name.filter(|name| !name.as_bytes().is_empty());
        let (true, Some(name), false) = (cookie, name, partitioned) else {
            return Err(InstructionsError::Credential);
        };
        let slot = credentials.get_mut(count).ok_or(InstructionsError::TooLarge)?;
        *slot = Some(name);
        count = count.saturating_add(1);
        Ok(())
    })?;
    Ok(credentials)
}
