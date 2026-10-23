//! `neottx sound --page MPP INPUT.wav OUTPUT.t42` (REQ-CLI-011,
//! REQ-AUD-020): a sound as the audio profile's broadcast packets, with the
//! command that plays it, ready to add to a T42 stream. With `--live`, the
//! sound is sent as a live one (REQ-AUD-014), each block at the moment a
//! player of the file reaches it.

#[cfg(not(feature = "sound"))]
use neoteletext_app::Failure;
#[cfg(not(feature = "sound"))]
use std::path::Path;

#[cfg(feature = "sound")]
mod run;
#[cfg(feature = "sound")]
mod wave;

#[cfg(feature = "sound")]
pub use run::sound;

/// The pace at which the browser plays a T42 file: 64 packets every 0.2
/// seconds, the engine's `DEFAULT_FETCH_INTERVAL_US`.
pub const PLAYER_PACKETS_PER_SECOND: u32 = 64 * 5;

/// What `neottx sound` writes. A build without `sound` refuses the command
/// before reading any of it.
#[derive(Clone, Debug)]
#[cfg_attr(not(feature = "sound"), allow(dead_code))]
pub struct SoundOptions {
    /// The page the sound belongs to, 100 to 8FF.
    pub page: String,
    /// The channel, 0 to 3.
    pub channel: u8,
    /// The gain, 0 to 100 per cent.
    pub gain: u8,
    /// `ima` or `pcm`.
    pub codec: String,
    /// The sound's handle.
    pub handle: u8,
    /// Repeat until stopped.
    pub looped: bool,
    /// Times the whole sequence is written, as a carousel repeats it.
    pub repeat: u8,
    /// Send the sound live, paced for a player.
    pub live: bool,
    /// The packets a second the player reads.
    pub pace: u32,
}

/// `neottx sound` in a build without it.
#[cfg(not(feature = "sound"))]
pub fn sound(_: &SoundOptions, _: &Path, _: &Path, _: bool) -> Result<(), Failure> {
    Err(neoteletext_app::not_in_build("writing sounds", "sound"))
}
