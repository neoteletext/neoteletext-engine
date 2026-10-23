//! Device Bound Session Credentials (REQ-WEB-017, decision E.110), the
//! W3C Web Application Security Working Group's Editor's Draft of 8
//! September 2026 (<https://w3c.github.io/webappsec-dbsc/>), sans-IO and
//! without allocation: the headers and the session instructions read, the
//! proofs written, and the sessions kept. The agent (`agent`) makes the
//! requests; the port keeps the keys ([`DeviceKeys`]), which never leave
//! the device: this crate sees only their public halves and signatures.
//!
//! A server asks for a session with `Secure-Session-Registration` on a
//! response; the agent creates a key and posts a proof to the path named.
//! The server's answer, the session instructions, names the cookies the
//! session keeps and the address that refreshes them. When a request in
//! the session's scope would go without one of those cookies, the agent
//! first asks the refresh address for new ones, with a proof signed by
//! the same key, and then sends the request.

use crate::bytes::{Overflow, TextBuf, Writer};
use crate::json::{JsonReader, JsonStr, Token};
use crate::url::{Origin, Url};

/// The registration header (section 9.2).
pub const REGISTRATION_HEADER: &[u8] = b"secure-session-registration";
/// The challenge header (section 9.4).
pub const CHALLENGE_HEADER: &[u8] = b"secure-session-challenge";
/// The proof a request carries (section 9.10).
pub const RESPONSE_HEADER: &[u8] = b"Secure-Session-Response";
/// The session a refresh names (section 9.9).
pub const SESSION_ID_HEADER: &[u8] = b"Sec-Secure-Session-Id";
/// The JWT type of every proof (section 9.10).
pub const PROOF_TYPE: &[u8] = b"dbsc+jwt";
/// The one algorithm offered: ECDSA on P-256 with SHA-256 (RFC 7518
/// section 3.4), which every platform's protected key storage has.
pub const ALGORITHM: &[u8] = b"ES256";

/// Sessions kept at once; a registration with every slot taken is not
/// made.
pub const MAX_SESSIONS: usize = 4;
/// The longest session identifier kept.
pub const MAX_SESSION_ID: usize = 64;
/// The longest challenge kept.
pub const MAX_CHALLENGE: usize = 128;
/// The longest `authorization` value kept.
pub const MAX_AUTHORIZATION: usize = 256;
/// The longest registration path or refresh address kept.
pub const MAX_SESSION_URL: usize = 512;
/// The cookies one session may name.
pub const MAX_CREDENTIALS: usize = 4;
/// The longest cookie name a session may name.
pub const MAX_CREDENTIAL_NAME: usize = 64;
/// The scope rules one session may give.
pub const MAX_SCOPE_RULES: usize = 8;
/// The longest domain or path of a scope rule.
pub const MAX_RULE_TEXT: usize = 128;
/// The longest session instructions read.
pub const MAX_INSTRUCTIONS_BYTES: usize = 8 * 1024;
/// How long a session lasts from its registration or its last refresh:
/// the longest a cookie may last (RFC 6265bis section 5.6.1, 400 days), as
/// section 8.9 recommends.
pub const SESSION_LIFETIME_SECONDS: u64 = 400 * 24 * 60 * 60;

/// Bytes of one coordinate of a P-256 public key, and of each half of an
/// ES256 signature (RFC 7518 section 3.4).
pub const COORDINATE_BYTES: usize = 32;
/// An ES256 signature: R then S, each 32 bytes, big-endian.
pub const SIGNATURE_BYTES: usize = 2 * COORDINATE_BYTES;

/// The port's name for one of its keys.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct KeyHandle(pub u32);

/// The public half of a P-256 key: its affine coordinates, big-endian.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PublicKey {
    /// x.
    pub x: [u8; COORDINATE_BYTES],
    /// y.
    pub y: [u8; COORDINATE_BYTES],
}

/// Keys that never leave the device (REQ-WEB-017): the optional part of
/// the platform interface a port with protected key storage supplies
/// (REQ-ARCH-009). Each key is a P-256 key pair whose private half the
/// port keeps where it cannot be read: a secure element, a trusted
/// platform module, the operating system's key store. It has no `Debug`
/// supertrait: a trait object's table would carry `core::fmt` into every
/// build (REQ-GEN-016).
pub trait DeviceKeys {
    /// Makes a new key pair and gives its handle and public half, or `None`
    /// when no key can be made.
    fn create(&mut self) -> Option<(KeyHandle, PublicKey)>;
    /// Signs `message` with ECDSA on P-256 and SHA-256, giving R then S,
    /// or `None` when the key is gone or cannot sign.
    fn sign(&mut self, key: KeyHandle, message: &[u8]) -> Option<[u8; SIGNATURE_BYTES]>;
    /// Destroys a key no session uses any more.
    fn delete(&mut self, key: KeyHandle);
}

// ---------------------------------------------------------------------
// Structured fields (RFC 9651), as much as the two headers use
// ---------------------------------------------------------------------

/// A cursor over a header value.
struct Fields<'a> {
    input: &'a [u8],
    at: usize,
}

impl<'a> Fields<'a> {
    const fn new(input: &'a [u8]) -> Self {
        Self { input, at: 0 }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.at).copied()
    }

    fn bump(&mut self) {
        self.at = self.at.saturating_add(1);
    }

    fn spaces(&mut self) {
        while self.peek() == Some(b' ') {
            self.bump();
        }
    }

    fn optional_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t')) {
            self.bump();
        }
    }

    fn at_end(&self) -> bool {
        self.at >= self.input.len()
    }

    /// An sf-string (RFC 9651 section 4.2.5), decoded into `out`.
    fn string<const N: usize>(&mut self, out: &mut TextBuf<N>) -> Option<()> {
        if self.peek() != Some(b'"') {
            return None;
        }
        self.bump();
        out.clear();
        loop {
            let byte = self.peek()?;
            self.bump();
            match byte {
                b'"' => return Some(()),
                b'\\' => {
                    let escaped = self.peek().filter(|&next| next == b'"' || next == b'\\')?;
                    self.bump();
                    out.push(&[escaped]).ok()?;
                }
                0x20..=0x7E => out.push(&[byte]).ok()?,
                _ => return None,
            }
        }
    }

    /// An sf-token (section 4.2.6), as the bytes it is.
    fn token(&mut self) -> Option<&'a [u8]> {
        let start = self.at;
        if !self.peek().is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'*') {
            return None;
        }
        while self.peek().is_some_and(|byte| is_tchar(byte) || byte == b':' || byte == b'/') {
            self.bump();
        }
        self.input.get(start..self.at)
    }

    /// A key (section 4.2.3.3).
    fn key(&mut self) -> Option<&'a [u8]> {
        let start = self.at;
        if !self.peek().is_some_and(|byte| byte.is_ascii_lowercase() || byte == b'*') {
            return None;
        }
        while self.peek().is_some_and(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.' | b'*')
        }) {
            self.bump();
        }
        self.input.get(start..self.at)
    }

    /// Steps over a bare item of any kind but a string, which the caller
    /// reads: a token, a number, a boolean or a byte sequence.
    fn skip_other_item(&mut self) -> Option<()> {
        match self.peek()? {
            b'?' => {
                self.bump();
                self.peek().filter(|&byte| byte == b'0' || byte == b'1')?;
                self.bump();
            }
            b':' => {
                self.bump();
                while self.peek()? != b':' {
                    self.bump();
                }
                self.bump();
            }
            byte if byte == b'-' || byte.is_ascii_digit() => {
                self.bump();
                while self.peek().is_some_and(|byte| byte.is_ascii_digit() || byte == b'.') {
                    self.bump();
                }
            }
            _ => {
                self.token()?;
            }
        }
        Some(())
    }

    /// Parameters (section 4.2.3.2), each given to `each` with its value
    /// when that is a string: `each(key, Some(fields))` lets the caller read
    /// the string; anything else is stepped over.
    fn parameters(&mut self, mut each: impl FnMut(&[u8], &mut Self) -> Option<bool>) -> Option<()> {
        while self.peek() == Some(b';') {
            self.bump();
            self.spaces();
            let key = self.key()?;
            if self.peek() != Some(b'=') {
                continue;
            }
            self.bump();
            if self.peek() == Some(b'"') {
                // The caller takes the string, or it is stepped over.
                let mut skipped = TextBuf::<MAX_SESSION_URL>::new();
                if !each(key, self)? {
                    self.string(&mut skipped)?;
                }
            } else {
                self.skip_other_item()?;
            }
        }
        Some(())
    }
}

/// RFC 9110 section 5.6.2's token characters.
const fn is_tchar(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~'
        )
}

/// What `Secure-Session-Registration` asks for (section 9.2): the first
/// member of the list that offers ES256 and names a path.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Registration {
    /// The registration endpoint, relative to the response's address.
    pub path: TextBuf<MAX_SESSION_URL>,
    /// The challenge for the proof's `jti`, when given.
    pub challenge: Option<TextBuf<MAX_CHALLENGE>>,
    /// A value for the proof's `authorization` and the request's
    /// `Authorization` header, when given.
    pub authorization: Option<TextBuf<MAX_AUTHORIZATION>>,
}

/// Reads a `Secure-Session-Registration` value, or `None` when no member
/// offers ES256 with a path, or the value breaks RFC 9651.
#[must_use]
pub fn read_registration(value: &[u8]) -> Option<Registration> {
    let mut fields = Fields::new(value);
    fields.spaces();
    let mut chosen = None;
    while !fields.at_end() {
        // A member: an inner list of algorithm tokens, then parameters.
        if fields.peek() != Some(b'(') {
            return None;
        }
        fields.bump();
        let mut offers_es256 = false;
        loop {
            fields.spaces();
            if fields.peek() == Some(b')') {
                fields.bump();
                break;
            }
            let token = fields.token()?;
            offers_es256 |= token == ALGORITHM;
            fields.parameters(|_, _| Some(false))?;
            if !matches!(fields.peek(), Some(b' ' | b')')) {
                return None;
            }
        }
        let mut path = None;
        let mut challenge = None;
        let mut authorization = None;
        fields.parameters(|key, fields| match key {
            b"path" => {
                let mut text = TextBuf::new();
                fields.string(&mut text)?;
                path = Some(text);
                Some(true)
            }
            b"challenge" => {
                let mut text = TextBuf::new();
                fields.string(&mut text)?;
                challenge = Some(text);
                Some(true)
            }
            b"authorization" => {
                let mut text = TextBuf::new();
                fields.string(&mut text)?;
                authorization = Some(text);
                Some(true)
            }
            _ => Some(false),
        })?;
        if chosen.is_none()
            && offers_es256
            && let Some(path) = path
        {
            chosen = Some(Registration { path, challenge, authorization });
        }
        fields.optional_whitespace();
        if fields.at_end() {
            break;
        }
        if fields.peek() != Some(b',') {
            return None;
        }
        fields.bump();
        fields.optional_whitespace();
        if fields.at_end() {
            return None;
        }
    }
    chosen
}

/// A `Secure-Session-Challenge` (section 9.4): a challenge for the session
/// it names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Challenge {
    /// The challenge.
    pub challenge: TextBuf<MAX_CHALLENGE>,
    /// The session's identifier.
    pub session: TextBuf<MAX_SESSION_ID>,
}

/// Reads a `Secure-Session-Challenge` value, or `None` when it is not an
/// sf-string with an `id` string parameter.
#[must_use]
pub fn read_challenge(value: &[u8]) -> Option<Challenge> {
    let mut fields = Fields::new(value);
    fields.spaces();
    let mut challenge = TextBuf::new();
    fields.string(&mut challenge)?;
    let mut session = None;
    fields.parameters(|key, fields| {
        if key != b"id" {
            return Some(false);
        }
        let mut text = TextBuf::new();
        fields.string(&mut text)?;
        session = Some(text);
        Some(true)
    })?;
    fields.spaces();
    (fields.at_end() && session.is_some()).then_some(Challenge { challenge, session: session? })
}

// ---------------------------------------------------------------------
// Session instructions (section 9.6)
// ---------------------------------------------------------------------

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
/// `member` reads the value or returns false to have it skipped.
fn object<'a>(
    first: Token<'a>,
    reader: &mut JsonReader<'a>,
    mut member: impl FnMut(JsonStr<'a>, &mut JsonReader<'a>) -> Read<bool>,
) -> Read<()> {
    if first != Token::BeginObject {
        return Err(InstructionsError::Missing);
    }
    loop {
        match next(reader)? {
            Token::EndObject => return Ok(()),
            Token::Key(name) => {
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
    object(first, &mut reader, |name, reader| {
        if name.equals(b"session_identifier") {
            id = Some(string(reader)?);
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
    object(first, reader, |name, reader| {
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
    object(first, reader, |name, reader| {
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
        object(first, reader, |member, reader| {
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

// ---------------------------------------------------------------------
// Proofs (section 9.10)
// ---------------------------------------------------------------------

/// Why a proof could not be written.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProofError {
    /// It did not fit.
    Overflow,
    /// The port could not sign.
    Signing,
}

impl From<Overflow> for ProofError {
    fn from(Overflow: Overflow) -> Self {
        Self::Overflow
    }
}

/// The base64url alphabet without padding (RFC 4648 section 5), as JWS
/// uses it (RFC 7515 section 2).
const BASE64URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
/// Bytes in a group of four base64 characters.
const BASE64_GROUP: usize = 3;
/// Where each byte of a group sits in its 24 bits, and where each of the
/// four characters' six bits sit (RFC 4648 section 4).
const BYTE_SHIFTS: [u32; BASE64_GROUP] = [16, 8, 0];
const SEXTET_SHIFTS: [u32; BASE64_GROUP + 1] = [18, 12, 6, 0];
const SEXTET_MASK: u32 = 0x3F;

/// Writes `bytes` in base64url without padding: a group of n bytes, read as
/// the high bits of 24, gives n + 1 characters.
fn base64url(bytes: &[u8], out: &mut Writer<'_>) -> Result<(), Overflow> {
    for group in bytes.chunks(BASE64_GROUP) {
        let value =
            group.iter().zip(BYTE_SHIFTS).fold(0_u32, |value, (&byte, shift)| value | (u32::from(byte) << shift));
        for shift in SEXTET_SHIFTS.iter().take(group.len().saturating_add(1)) {
            let sextet = (value >> shift) & SEXTET_MASK;
            out.byte(BASE64URL.get(sextet as usize).copied().unwrap_or(b'A'))?;
        }
    }
    Ok(())
}

/// Writes `text` as the inside of a JSON string: `"` and `\` escaped, as
/// the sf-strings it comes from hold only printable ASCII.
fn json_text(text: &[u8], out: &mut Writer<'_>) -> Result<(), Overflow> {
    for &byte in text {
        if byte == b'"' || byte == b'\\' {
            out.byte(b'\\')?;
        }
        out.byte(byte)?;
    }
    Ok(())
}

/// Room for a proof's header and payload before they are encoded.
const PROOF_JSON_BYTES: usize = 1024;

/// Writes a proof (section 9.10) to `out`: a JWS in compact form, its
/// header `{"alg":"ES256","typ":"dbsc+jwt"}` with the public key as `jwk`
/// at registration, its payload `{"jti": challenge}` (`{}` when the
/// server gave no challenge) with `authorization` when given, signed by
/// `key` through `keys`.
///
/// # Errors
/// [`ProofError`].
pub fn write_proof(
    keys: &mut dyn DeviceKeys,
    key: KeyHandle,
    public: Option<&PublicKey>,
    challenge: Option<&[u8]>,
    authorization: Option<&[u8]>,
    out: &mut Writer<'_>,
) -> Result<(), ProofError> {
    let start = out.len();
    let mut json = [0_u8; PROOF_JSON_BYTES];
    let mut header = Writer::new(&mut json);
    header.put(b"{\"alg\":\"")?;
    header.put(ALGORITHM)?;
    header.put(b"\",\"typ\":\"")?;
    header.put(PROOF_TYPE)?;
    header.byte(b'"')?;
    if let Some(public) = public {
        // RFC 7518 section 6.2.1: an elliptic-curve public key.
        header.put(b",\"jwk\":{\"kty\":\"EC\",\"crv\":\"P-256\",\"x\":\"")?;
        base64url(&public.x, &mut header)?;
        header.put(b"\",\"y\":\"")?;
        base64url(&public.y, &mut header)?;
        header.put(b"\"}")?;
    }
    header.byte(b'}')?;
    base64url(header.written(), out)?;
    out.byte(b'.')?;
    let mut json = [0_u8; PROOF_JSON_BYTES];
    let mut payload = Writer::new(&mut json);
    payload.byte(b'{')?;
    if let Some(challenge) = challenge {
        payload.put(b"\"jti\":\"")?;
        json_text(challenge, &mut payload)?;
        payload.byte(b'"')?;
    }
    if let Some(authorization) = authorization {
        if challenge.is_some() {
            payload.byte(b',')?;
        }
        payload.put(b"\"authorization\":\"")?;
        json_text(authorization, &mut payload)?;
        payload.byte(b'"')?;
    }
    payload.byte(b'}')?;
    base64url(payload.written(), out)?;
    let signing_input = out.written().get(start..).unwrap_or(&[]);
    let signature = keys.sign(key, signing_input).ok_or(ProofError::Signing)?;
    out.byte(b'.')?;
    base64url(&signature, out)?;
    Ok(())
}

// ---------------------------------------------------------------------
// Sessions (sections 8.5, 8.6 and 8.9)
// ---------------------------------------------------------------------

/// One bound session.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Session {
    /// Its identifier.
    pub id: TextBuf<MAX_SESSION_ID>,
    /// The key that proves it.
    pub key: KeyHandle,
    /// Where it is refreshed.
    pub refresh_url: TextBuf<MAX_SESSION_URL>,
    /// The origin whose requests it covers, or whose site's.
    pub origin: Origin,
    /// Site scope: the origin's host and every host below it.
    pub include_site: bool,
    /// The scope rules, applied last first.
    pub rules: [Option<ScopeRule>; MAX_SCOPE_RULES],
    /// The cookies it keeps.
    pub credentials: [Option<TextBuf<MAX_CREDENTIAL_NAME>>; MAX_CREDENTIALS],
    /// The last challenge the server gave it.
    pub challenge: Option<TextBuf<MAX_CHALLENGE>>,
    /// When it lapses, in the agent's seconds.
    pub expires: u64,
}

/// True when `host` matches a rule's `domain`: `*`, `*.example.com` (any
/// host below `example.com`), or the host itself, without regard to case.
fn domain_matches(pattern: &[u8], host: &[u8]) -> bool {
    if pattern == b"*" {
        return true;
    }
    if let Some(parent) = pattern.strip_prefix(b"*.") {
        return host.len() > parent.len()
            && host
                .get(host.len().saturating_sub(parent.len())..)
                .is_some_and(|tail| tail.eq_ignore_ascii_case(parent))
            && host.get(host.len().saturating_sub(parent.len()).saturating_sub(1)) == Some(&b'.');
    }
    pattern.eq_ignore_ascii_case(host)
}

impl Session {
    /// Whether `url` is in this session's scope (section 8.5): its origin,
    /// or with site scope its host and those below, over HTTPS; then the
    /// last scope rule that matches decides, and none means in scope. The
    /// refresh address itself never is, so that a refresh is never
    /// deferred for itself.
    #[must_use]
    pub fn covers(&self, url: &Url) -> bool {
        let host = url.host_text().as_bytes();
        let within = if self.include_site {
            url.is_secure() && self.origin.secure && site_member(host, self.origin.host.as_bytes())
        } else {
            url.origin() == self.origin
        };
        if !within || url.as_str().as_bytes() == self.refresh_url.as_bytes() {
            return false;
        }
        let path = url.path().as_bytes();
        self.rules
            .iter()
            .rev()
            .flatten()
            .find(|rule| domain_matches(rule.domain.as_bytes(), host) && path.starts_with(rule.path.as_bytes()))
            .is_none_or(|rule| rule.include)
    }

    /// Whether a request carrying the cookie header `cookies` (its value,
    /// `name=value; ...`) lacks one of the session's cookies (section 8.6),
    /// so that the session must be refreshed first.
    #[must_use]
    pub fn missing(&self, cookies: &[u8]) -> bool {
        self.credentials.iter().flatten().any(|name| {
            !cookies.split(|&byte| byte == b';').any(|pair| {
                let pair = crate::bytes::trim_ows(pair);
                pair.split(|&byte| byte == b'=').next() == Some(name.as_bytes())
            })
        })
    }
}

/// True when `host` is `site` or a host below it.
fn site_member(host: &[u8], site: &[u8]) -> bool {
    if host.eq_ignore_ascii_case(site) {
        return true;
    }
    host.len() > site.len()
        && host.get(host.len().saturating_sub(site.len())..).is_some_and(|tail| tail.eq_ignore_ascii_case(site))
        && host.get(host.len().saturating_sub(site.len()).saturating_sub(1)) == Some(&b'.')
}

/// The sessions an agent keeps.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Sessions {
    slots: [Option<Session>; MAX_SESSIONS],
}

impl Default for Sessions {
    fn default() -> Self {
        Self::new()
    }
}

impl Sessions {
    /// None.
    #[must_use]
    pub const fn new() -> Self {
        Self { slots: [None; MAX_SESSIONS] }
    }

    /// The sessions, by slot.
    pub fn iter(&self) -> impl Iterator<Item = (usize, &Session)> {
        self.slots.iter().enumerate().filter_map(|(index, slot)| slot.as_ref().map(|session| (index, session)))
    }

    /// The session in `slot`.
    #[must_use]
    pub fn get(&self, slot: usize) -> Option<&Session> {
        self.slots.get(slot).and_then(Option::as_ref)
    }

    /// The session in `slot`, to change.
    pub fn get_mut(&mut self, slot: usize) -> Option<&mut Session> {
        self.slots.get_mut(slot).and_then(Option::as_mut)
    }

    /// The slot of the session `id` of `origin`.
    #[must_use]
    pub fn find(&self, origin: &Origin, id: &[u8]) -> Option<usize> {
        self.iter().find(|(_, session)| session.origin == *origin && session.id.as_bytes() == id).map(|(slot, _)| slot)
    }

    /// True when a new session could be kept.
    #[must_use]
    pub fn has_room(&self) -> bool {
        self.slots.iter().any(Option::is_none)
    }

    /// Keeps `session`, replacing one of the same origin and identifier;
    /// gives back the key of the session it replaced, or as the error the
    /// new session's own key when there is no room, for the caller to
    /// delete.
    pub fn insert(&mut self, session: Session) -> Result<Option<KeyHandle>, KeyHandle> {
        if let Some(slot) = self.find(&session.origin, session.id.as_bytes()) {
            let old = self.slots.get_mut(slot).and_then(|slot| slot.replace(session));
            return Ok(old.map(|old| old.key).filter(|&key| key != session.key));
        }
        let Some(slot) = self.slots.iter_mut().find(|slot| slot.is_none()) else { return Err(session.key) };
        *slot = Some(session);
        Ok(None)
    }

    /// Ends the session in `slot`, giving it back so its key can be
    /// deleted.
    pub fn remove(&mut self, slot: usize) -> Option<Session> {
        self.slots.get_mut(slot).and_then(Option::take)
    }

    /// Ends every session past its time, calling `ended` with each.
    pub fn expire(&mut self, now: u64, mut ended: impl FnMut(Session)) {
        for slot in &mut self.slots {
            if slot.is_some_and(|session| session.expires <= now)
                && let Some(session) = slot.take()
            {
                ended(session);
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, reason = "a test")]
mod tests {
    use super::*;
    use std::string::String;
    use std::vec::Vec;

    #[test]
    fn registration_offers_are_read() {
        let read =
            read_registration(br#"(ES256 RS256);path="/dbsc/start";challenge="c-1";authorization="code""#).unwrap();
        assert_eq!(read.path.as_str(), "/dbsc/start");
        assert_eq!(read.challenge.unwrap().as_str(), "c-1");
        assert_eq!(read.authorization.unwrap().as_str(), "code");
        // The first member offering ES256 is taken; others are read past.
        let read = read_registration(br#"(RS256);path="/rsa", (ES256);path="/ec";x=?1;n=5"#).unwrap();
        assert_eq!(read.path.as_str(), "/ec");
        assert_eq!(read.challenge, None);
        let escaped = read_registration(br#"(ES256);path="/a";challenge="q\"b\\c""#).unwrap();
        assert_eq!(escaped.challenge.unwrap().as_str(), r#"q"b\c"#);
        for refused in [
            &b"(RS256);path=\"/rsa\""[..],
            b"(ES256)",
            b"(ES256);path=/bare",
            b"ES256;path=\"/x\"",
            b"(ES256);path=\"/x\",",
            b"(ES256);path=\"unterminated",
            b"(ES256);path=\"tab\there\"",
            b"(ES256);challenge=\"\\x\";path=\"/x\"",
        ] {
            assert_eq!(read_registration(refused), None, "{}", String::from_utf8_lossy(refused));
        }
        let long = [b"(ES256);path=\"".as_slice(), &[b'a'; MAX_SESSION_URL + 1], b"\""].concat();
        assert_eq!(read_registration(&long), None, "a path longer than kept");
    }

    #[test]
    fn challenges_name_their_session() {
        let read = read_challenge(br#""fresh";id="s1""#).unwrap();
        assert_eq!((read.challenge.as_str(), read.session.as_str()), ("fresh", "s1"));
        assert_eq!(read_challenge(br#""fresh""#), None, "no id");
        assert_eq!(read_challenge(br#""fresh";id=s1"#), None, "a token id");
        assert_eq!(read_challenge(br#""a", "b";id="s""#), None, "a list");
    }

    const INSTRUCTIONS: &[u8] = br#"{
        "session_identifier": "s1",
        "refresh_url": "/dbsc/refresh",
        "scope": {"origin": "https://news.example", "include_site": false,
                  "scope_specification": [{"type": "exclude", "domain": "news.example", "path": "/public"}]},
        "credentials": [{"type": "cookie", "name": "auth", "attributes": "Domain=news.example; Secure"}],
        "allowed_refresh_initiators": ["*.news.example"],
        "newer_member": {"ignored": [1, 2, 3]}
    }"#;

    #[test]
    fn instructions_are_read() {
        let Ok(Answer::Continue(read)) = read_instructions(INSTRUCTIONS) else { panic!() };
        assert_eq!(read.id.as_str(), "s1");
        assert_eq!(read.refresh_url.unwrap().as_str(), "/dbsc/refresh");
        assert_eq!(read.origin.unwrap().as_str(), "https://news.example");
        assert!(!read.include_site);
        let rule = read.rules[0].unwrap();
        assert_eq!((rule.include, rule.domain.as_str(), rule.path.as_str()), (false, "news.example", "/public"));
        assert_eq!(read.credentials[0].unwrap().as_str(), "auth");
        assert_eq!(read_instructions(br#"{"continue": false}"#), Ok(Answer::End));
    }

    #[test]
    fn instructions_outside_the_profile_are_refused() {
        let base =
            |members: &str| std::format!(r#"{{"session_identifier":"s","scope":{{"include_site":true}},{members}}}"#);
        for (body, why) in [
            (String::from("{"), InstructionsError::Malformed),
            (String::from(r#"{"continue":true} trailing"#), InstructionsError::Malformed),
            (base(r#""credentials":[]"#), InstructionsError::Missing),
            (base(r#""credentials":[{"type":"token","name":"a"}]"#), InstructionsError::Credential),
            (base(r#""credentials":[{"type":"cookie","name":""}]"#), InstructionsError::Credential),
            (
                base(r#""credentials":[{"type":"cookie","name":"a","attributes":"Secure; Partitioned"}]"#),
                InstructionsError::Credential,
            ),
            (
                r#"{"scope":{"include_site":true},"credentials":[{"type":"cookie","name":"a"}]}"#.into(),
                InstructionsError::Missing,
            ),
            (
                r#"{"session_identifier":"s","scope":{},"credentials":[{"type":"cookie","name":"a"}]}"#.into(),
                InstructionsError::Missing,
            ),
            (
                base(&std::format!(
                    r#""credentials":[{}]"#,
                    [r#"{"type":"cookie","name":"a"}"#; MAX_CREDENTIALS + 1].join(",")
                )),
                InstructionsError::TooLarge,
            ),
        ] {
            assert_eq!(read_instructions(body.as_bytes()), Err(why), "{body}");
        }
        let long = std::format!(r#"{{"session_identifier":"{}"}}"#, "a".repeat(MAX_SESSION_ID + 1));
        assert_eq!(read_instructions(long.as_bytes()), Err(InstructionsError::TooLarge));
    }

    /// A key store that signs with a stand-in signature and records what
    /// it signed.
    #[derive(Debug)]
    struct Keys {
        signed: Vec<Vec<u8>>,
    }

    impl DeviceKeys for Keys {
        fn create(&mut self) -> Option<(KeyHandle, PublicKey)> {
            Some((KeyHandle(7), PublicKey { x: [1; COORDINATE_BYTES], y: [2; COORDINATE_BYTES] }))
        }
        fn sign(&mut self, key: KeyHandle, message: &[u8]) -> Option<[u8; SIGNATURE_BYTES]> {
            assert_eq!(key, KeyHandle(7));
            self.signed.push(message.to_vec());
            Some([0xAB; SIGNATURE_BYTES])
        }
        fn delete(&mut self, _: KeyHandle) {}
    }

    fn unbase64url(text: &[u8]) -> Vec<u8> {
        let mut standard: Vec<u8> = text
            .iter()
            .map(|&byte| match byte {
                b'-' => b'+',
                b'_' => b'/',
                other => other,
            })
            .collect();
        while !standard.len().is_multiple_of(4) {
            standard.push(b'=');
        }
        let mut out = std::vec![0_u8; standard.len()];
        let length = crate::base64::decode(&standard, &mut out).unwrap();
        out.truncate(length);
        out
    }

    #[test]
    fn proofs_are_compact_jws_signed_by_the_port() {
        let mut keys = Keys { signed: Vec::new() };
        let (key, public) = keys.create().unwrap();
        let mut buffer = [0_u8; 2048];
        let mut out = Writer::new(&mut buffer);
        out.put(b"Secure-Session-Response: ").unwrap();
        let start = out.len();
        write_proof(&mut keys, key, Some(&public), Some(br#"c"1"#), Some(b"code"), &mut out).unwrap();
        let jws = out.written()[start..].to_vec();
        let parts: Vec<&[u8]> = jws.split(|&byte| byte == b'.').collect();
        assert_eq!(parts.len(), 3);
        let header = String::from_utf8(unbase64url(parts[0])).unwrap();
        let x = String::from_utf8(parts[0].to_vec()).unwrap();
        assert!(!x.contains('=') && !x.contains('+') && !x.contains('/'), "base64url without padding");
        assert_eq!(
            header,
            std::format!(
                r#"{{"alg":"ES256","typ":"dbsc+jwt","jwk":{{"kty":"EC","crv":"P-256","x":"{x}","y":"{y}"}}}}"#,
                x = "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE",
                y = "AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgI"
            )
        );
        assert_eq!(String::from_utf8(unbase64url(parts[1])).unwrap(), r#"{"jti":"c\"1","authorization":"code"}"#);
        assert_eq!(unbase64url(parts[2]), [0xAB; SIGNATURE_BYTES]);
        // What was signed is the JWS signing input (RFC 7515 section 5.1).
        assert_eq!(keys.signed, [[parts[0], b".", parts[1]].concat()]);

        let mut buffer = [0_u8; 512];
        let mut out = Writer::new(&mut buffer);
        write_proof(&mut keys, key, None, Some(b"r"), None, &mut out).unwrap();
        let parts: Vec<Vec<u8>> = out.written().split(|&byte| byte == b'.').map(<[u8]>::to_vec).collect();
        assert_eq!(String::from_utf8(unbase64url(&parts[0])).unwrap(), r#"{"alg":"ES256","typ":"dbsc+jwt"}"#);
        assert_eq!(String::from_utf8(unbase64url(&parts[1])).unwrap(), r#"{"jti":"r"}"#);
    }

    fn session(include_site: bool, rules: &[(bool, &str, &str)]) -> Session {
        let mut kept = [None; MAX_SCOPE_RULES];
        for (slot, &(include, domain, path)) in kept.iter_mut().zip(rules) {
            *slot = Some(ScopeRule {
                include,
                domain: TextBuf::from_bytes(domain.as_bytes()).unwrap(),
                path: TextBuf::from_bytes(path.as_bytes()).unwrap(),
            });
        }
        let mut credentials = [None; MAX_CREDENTIALS];
        credentials[0] = TextBuf::from_bytes(b"auth");
        Session {
            id: TextBuf::from_bytes(b"s1").unwrap(),
            key: KeyHandle(7),
            refresh_url: TextBuf::from_bytes(b"https://news.example/dbsc/refresh").unwrap(),
            origin: Url::parse("https://news.example/").unwrap().origin(),
            include_site,
            rules: kept,
            credentials,
            challenge: None,
            expires: 100,
        }
    }

    #[test]
    fn scope_is_the_origin_or_the_site_less_what_rules_exclude() {
        let covers = |session: &Session, url: &str| session.covers(&Url::parse(url).unwrap());
        let origin = session(false, &[]);
        assert!(covers(&origin, "https://news.example/p/100.tti"));
        assert!(!covers(&origin, "https://www.news.example/p/100.tti"), "another origin");
        assert!(!covers(&origin, "http://news.example/p/100.tti"), "plain HTTP");
        assert!(!covers(&origin, "https://news.example/dbsc/refresh"), "the refresh address itself");
        let site = session(true, &[(false, "*", "/public"), (true, "*", "/public/mine")]);
        assert!(covers(&site, "https://www.news.example/p/100.tti"));
        assert!(!covers(&site, "https://othernews.example/p"), "not below the site");
        assert!(covers(&site, "https://news.example/public/mine/1"), "the last matching rule decides");
        assert!(!covers(&site, "https://news.example/public/other"));
        let excluded = session(true, &[(false, "*.news.example", "/")]);
        assert!(!covers(&excluded, "https://cdn.news.example/a") && covers(&excluded, "https://news.example/a"));
    }

    #[test]
    fn a_missing_cookie_is_found_by_name() {
        let session = session(false, &[]);
        assert!(!session.missing(b"theme=dark; auth=xyz"));
        assert!(session.missing(b"theme=dark"));
        assert!(session.missing(b"author=1; xauth=2"));
        assert!(session.missing(b""));
    }

    #[test]
    fn the_store_is_bounded_and_replaces_by_identifier() {
        let mut sessions = Sessions::new();
        let mut first = session(false, &[]);
        assert_eq!(sessions.insert(first), Ok(None));
        first.key = KeyHandle(8);
        assert_eq!(sessions.insert(first), Ok(Some(KeyHandle(7))), "the replaced key is the caller's to delete");
        for index in 1..MAX_SESSIONS {
            let mut other = session(false, &[]);
            other.id = TextBuf::from_bytes(std::format!("s{index}x").as_bytes()).unwrap();
            assert_eq!(sessions.insert(other), Ok(None));
        }
        let mut more = session(false, &[]);
        more.id = TextBuf::from_bytes(b"over").unwrap();
        more.key = KeyHandle(9);
        assert_eq!(sessions.insert(more), Err(KeyHandle(9)));
        let mut ended = Vec::new();
        sessions.expire(100, |session| ended.push(session.key));
        assert_eq!(ended.len(), MAX_SESSIONS);
        assert!(sessions.iter().next().is_none());
    }
}
