//! The two response headers of Device Bound Session Credentials: what
//! `Secure-Session-Registration` asks for (section 9.2) and the challenge
//! `Secure-Session-Challenge` gives (section 9.4).

use super::structured::Fields;
use super::{ALGORITHM, MAX_AUTHORIZATION, MAX_CHALLENGE, MAX_SESSION_ID, MAX_SESSION_URL};
use crate::bytes::TextBuf;

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
