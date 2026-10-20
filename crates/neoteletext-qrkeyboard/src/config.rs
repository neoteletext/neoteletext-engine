//! The screen's configuration, as qrkeyboard's screen SDK builds it
//! (`v1.ts` `#buildConfig`) and the relay stores it (`index.ts`
//! `withDefaults`), so that the hash the phone computes over what the
//! relay gives it equals the hash in the QR (`crypto.ts` `configHash`).

use sha2::{Digest, Sha256};

use crate::json::Value;
use crate::{Error, PROTOCOL_VERSION};

/// The hash in the QR: SHA-256 of the canonical configuration, cut to 128
/// bits (`CONFIG_HASH_BYTES`).
pub const CONFIG_HASH_BYTES: usize = 16;
/// The longest screen name, in UTF-16 code units (`MAX_NAME_LENGTH`).
pub const MAX_NAME_LENGTH: usize = 120;
/// The longest a dropped phone may keep the screen, in seconds (the
/// relay's `MAX_REJOIN_SECONDS`).
pub const MAX_REJOIN_SECONDS: u32 = 3600;
/// The SDK's default submit label, which `live` mode never shows.
const SUBMIT_LABEL: &str = "Send";

/// The phone's controls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Layout {
    /// One of qrkeyboard's presets: `keyboard`, `remote`, `teletext` or
    /// `gamepad` (`controls.ts` `PRESETS`).
    Preset(&'static str),
    /// A text box, the direction pad and the four colour keys: a keyboard
    /// for typing into a page and for playing on one.
    Terminal,
}

/// The colour keys, as qrkeyboard names them (W3C `KeyboardEvent.key`).
const COLOUR_KEYS: [&str; 4] = ["ColorF0Red", "ColorF1Green", "ColorF2Yellow", "ColorF3Blue"];
/// The colour keys sit four to a row, as on a remote.
const COLOUR_COLUMNS: u64 = 4;

impl Layout {
    fn value(&self) -> Value {
        match self {
            Self::Preset(name) => Value::string(name),
            Self::Terminal => {
                let keys = COLOUR_KEYS.iter().map(|key| Value::object(&[("key", Value::string(key))])).collect();
                Value::Array(vec![
                    Value::object(&[("type", Value::string("text"))]),
                    Value::object(&[("type", Value::string("dpad"))]),
                    Value::object(&[
                        ("type", Value::string("keys")),
                        ("columns", Value::integer(COLOUR_COLUMNS)),
                        ("keys", Value::Array(keys)),
                    ]),
                ])
            }
        }
    }

    /// The capabilities the SDK derives from the layout: `keyboard` with a
    /// text block, `buttons` with keys.
    fn caps(&self) -> Vec<Value> {
        let (text, keys) = match self {
            Self::Preset("keyboard") => (true, false),
            Self::Preset(_) => (false, true),
            Self::Terminal => (true, true),
        };
        let mut caps = Vec::new();
        if text {
            caps.push(Value::string("keyboard"));
        }
        if keys {
            caps.push(Value::string("buttons"));
        }
        caps
    }
}

/// What the screen asks the relay for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreenConfig {
    /// The screen's name, shown on the phone.
    pub name: String,
    /// The phone's controls.
    pub layout: Layout,
    /// Pair with a PIN shown beside the QR, by SPAKE2, rather than with a
    /// key in the QR.
    pub pin: bool,
    /// How long a dropped phone keeps the screen, in seconds.
    pub rejoin_seconds: Option<u32>,
}

impl ScreenConfig {
    /// Checks the fields against the relay's limits.
    ///
    /// # Errors
    /// An empty or long name, or a rejoin time beyond the relay's limit.
    pub fn check(&self) -> Result<(), Error> {
        let units = self.name.encode_utf16().count();
        if units == 0 || units > MAX_NAME_LENGTH {
            return Err(Error::Limit("the screen's name"));
        }
        if self.rejoin_seconds.is_some_and(|seconds| seconds > MAX_REJOIN_SECONDS) {
            return Err(Error::Limit("the rejoin time"));
        }
        Ok(())
    }

    /// The configuration as the relay stores it and the phone reads it.
    #[must_use]
    pub fn value(&self) -> Value {
        let mut members = vec![
            ("caps", Value::Array(self.layout.caps())),
            ("mode", Value::string("live")),
            ("name", Value::string(&self.name)),
            ("submitLabel", Value::string(SUBMIT_LABEL)),
            ("protocol", Value::integer(u64::from(PROTOCOL_VERSION))),
            ("layout", self.layout.value()),
        ];
        if self.pin {
            members.push(("pinRequired", Value::Bool(true)));
        }
        if let Some(seconds) = self.rejoin_seconds {
            members.push(("rejoinSeconds", Value::integer(u64::from(seconds))));
        }
        Value::object(&members)
    }

    /// The configuration's hash, as the QR carries it.
    #[must_use]
    pub fn hash(&self) -> [u8; CONFIG_HASH_BYTES] {
        hash(&self.value())
    }
}

/// The hash of any configuration value (`crypto.ts` `configHash`).
#[must_use]
pub fn hash(config: &Value) -> [u8; CONFIG_HASH_BYTES] {
    let digest = Sha256::digest(config.canonical().as_bytes());
    let mut hash = [0_u8; CONFIG_HASH_BYTES];
    for (target, source) in hash.iter_mut().zip(digest.iter()) {
        *target = *source;
    }
    hash
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// The hash qrkeyboard's own `configHash` (`relay/src/crypto.ts`)
    /// computes for these configurations, by running it under Node on 20
    /// October 2026 (the teletext one again on 21 October, when its name
    /// became NeoTeletext).
    #[test]
    fn the_hash_is_the_sdks() {
        let teletext = ScreenConfig {
            name: "NeoTeletext".into(),
            layout: Layout::Preset("teletext"),
            pin: false,
            rejoin_seconds: Some(30),
        };
        assert_eq!(
            teletext.value().canonical(),
            r#"{"caps":["buttons"],"layout":"teletext","mode":"live","name":"NeoTeletext","protocol":3,"rejoinSeconds":30,"submitLabel":"Send"}"#
        );
        assert_eq!(teletext.hash(), FIXTURE_TELETEXT);
        let terminal =
            ScreenConfig { name: "Channel é".into(), layout: Layout::Terminal, pin: true, rejoin_seconds: None };
        assert_eq!(terminal.hash(), FIXTURE_TERMINAL);
    }

    #[test]
    fn the_fields_are_checked() {
        let mut config =
            ScreenConfig { name: String::new(), layout: Layout::Terminal, pin: false, rejoin_seconds: None };
        assert!(config.check().is_err());
        config.name = "x".repeat(MAX_NAME_LENGTH);
        assert!(config.check().is_ok());
        config.name.push('x');
        assert!(config.check().is_err());
        config.name = "x".into();
        config.rejoin_seconds = Some(MAX_REJOIN_SECONDS + 1);
        assert!(config.check().is_err());
    }

    const FIXTURE_TELETEXT: [u8; CONFIG_HASH_BYTES] =
        [0x99, 0xd8, 0xf7, 0xa6, 0xcc, 0xbe, 0xf2, 0xd0, 0xa8, 0x03, 0x34, 0x73, 0xda, 0x3b, 0x37, 0xe7];
    const FIXTURE_TERMINAL: [u8; CONFIG_HASH_BYTES] =
        [0xa1, 0x7a, 0x24, 0x4d, 0x26, 0xc6, 0x09, 0x36, 0x6c, 0x9b, 0xb4, 0xd8, 0x30, 0x48, 0xab, 0xcf];
}
