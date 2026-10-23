//! Device Bound Session Credentials' readers (REQ-TST-040, REQ-WEB-017):
//! the registration and challenge headers and the session instructions,
//! whose values must stay within their bounds.
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_web::dbsc::{
    Answer, MAX_CHALLENGE, MAX_SESSION_ID, MAX_SESSION_URL, read_challenge, read_instructions, read_registration,
};

fuzz_target!(|data: &[u8]| {
    if let Some(registration) = read_registration(data) {
        assert!(registration.path.as_bytes().len() <= MAX_SESSION_URL);
        assert!(registration.challenge.is_none_or(|challenge| challenge.as_bytes().len() <= MAX_CHALLENGE));
    }
    if let Some(challenge) = read_challenge(data) {
        assert!(challenge.session.as_bytes().len() <= MAX_SESSION_ID);
        // An sf-string decodes to printable ASCII only.
        assert!(challenge.challenge.as_bytes().iter().all(|byte| (0x20..=0x7E).contains(byte)));
    }
    if let Ok(Answer::Continue(instructions)) = read_instructions(data) {
        assert!(instructions.credentials.iter().any(Option::is_some));
        assert!(instructions.credentials.iter().flatten().all(|name| !name.as_bytes().is_empty()));
    }
});
