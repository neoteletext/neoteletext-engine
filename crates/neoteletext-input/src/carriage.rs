//! The channel's records for its input on the audio profile's carriage
//! (`INPUT.md` section 7): one fragment, on the audio data channel, of an
//! input record type.

/// Whether `packet` carries one of the channel's records for its input: a
/// cursor or control record, one fragment on the audio profile's data
/// channel (section 7). What it says is read by [`cursor::from_packet`](crate::cursor::from_packet) and
/// [`control::from_packet`](crate::control::from_packet).
#[must_use]
pub fn is_input_packet(packet: &neoteletext_core::packet::Packet) -> bool {
    matches!(
        neoteletext_audio::idl::fragment(packet),
        Some(Ok(fragment)) if fragment.first
            && fragment.last
            && fragment.data.get(1).is_some_and(|&kind| neoteletext_audio::record::is_input_type(kind))
    )
}
