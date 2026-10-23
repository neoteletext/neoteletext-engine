//! The channel's records for its input on the audio profile's carriage
//! (`INPUT.md` section 7): one fragment, on the audio data channel, of an
//! input record type.

use neoteletext_audio::idl::{Fragment, fragment};
use neoteletext_audio::record::is_input_type;
use neoteletext_core::packet::Packet;

/// Whether `packet` carries one of the channel's records for its input: a
/// cursor or control record, one fragment on the audio profile's data
/// channel (section 7). What it says is read by [`cursor::from_packet`](crate::cursor::from_packet) and
/// [`control::from_packet`](crate::control::from_packet).
#[must_use]
pub fn is_input_packet(packet: &Packet) -> bool {
    input_record(packet, is_input_type).is_some()
}

/// The fragment `packet` carries when it is a whole record, its first and
/// last fragment, of a type `kind` accepts: the test every reader of the
/// channel's input records makes (section 7).
pub(crate) fn input_record(packet: &Packet, kind: impl Fn(u8) -> bool) -> Option<Fragment> {
    let fragment = fragment(packet)?.ok()?;
    (fragment.first && fragment.last && fragment.data.get(1).copied().is_some_and(kind)).then_some(fragment)
}
