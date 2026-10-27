# F.22 Audio (AUD)

This area is the teletext audio profile (decisions E.84 to E.86): sounds carried as records on EN 300 708 independent data lines, over a network or in a file, kept compressed, and played on four channels by a voice sink the port supplies. The design, its sources and its departures from the standards are explained in `docs/spec/design/audio.md`; how each platform builds it is in `docs/spec/design/audio-platforms.md`.

**REQ-AUD-001 — Formats.**
*Phase:* Phase 2 (the whole area).
The profile shall carry two formats, and only these: code 0, linear PCM, signed 16-bit little-endian; and code 1, IMA ADPCM in WAVE blocks (format tag 0x0011), whose blocks each begin with their own predictor and step index. Every sound is one channel. A receiver shall ignore an object of any other code; codes 2 to 255 are reserved. *Amended 20 October 2026 (decision E.89):* code 2 was reserved for a speech codec for live broadcast; the owner decided there will be none. The IMA ADPCM decoder shall follow the IMA's 1992 recommendation, saturate rather than wrap, and refuse a block whose step index exceeds 88. *Amended 26 October 2026 (decision E.124):* code 2 is Opus in the profile of decision E.123, at 48 kHz alone; an object of code 2 at another rate is refused, and a receiver built without the decoder ignores code 2.
*Amended 28 October 2026 (decision E.131):* each codec is a feature of the receiver, `pcm16`, `ima` and `opus`, all built by default; the desktop builds (Windows, macOS and Linux) leave out PCM16 and IMA ADPCM, which microcontroller builds keep. A receiver built without a codec's feature shall treat an object of that codec as unsupported: ignored, as an object of a reserved code is, and counted, never played and never a fault.
*Verification:* A worked decoding of a known block; tests of saturation and of a bad step index; PCM round-trips exactly through the whole receiver; Opus objects read at 48 kHz and refused at another rate (`opus_objects_are_read_at_48_khz_with_packets_the_decoder_takes`); in every feature shape the gates build, a codec left out is read as unsupported and its sound skipped (`codecs_left_out_of_the_build_are_read_as_unsupported`, `a_pcm16_sound_plays_only_in_a_build_with_pcm16`, `an_ima_adpcm_sound_plays_only_in_a_build_with_ima`, `without_pcm16_a_pcm_sound_is_unsupported_and_skipped`).

**REQ-AUD-002 — Sample rates.**
A sound's rate shall be one of 8 000, 11 025, 16 000, 22 050, 24 000, 32 000, 44 100 and 48 000 samples a second, named by the codes 0 to 7. Any other code makes the OBJECT record invalid.
*Verification:* Tests of every code and of a code beyond the table; `neottx sound` refuses a WAVE file at another rate.

**REQ-AUD-003 — Records.**
Every carriage shall deliver records of at most 1 024 bytes: the profile version (2), a record type, a 16-bit body length of at most 1 016, the body, and a CRC-32/ISO-HDLC over the header and body (check value 0xCBF43926 for ASCII `123456789`). Integers are big-endian. A record shall be refused whole on a wrong length, trailing bytes, an unknown version or type, a failed CRC or any field outside its range, before anything in it is stored.
*Verification:* Tests of each refusal and of the CRC's check value; the fuzz target `audio_records`.

**REQ-AUD-004 — The OBJECT record.**
An OBJECT record shall carry the sound's handle (0 to 255), its generation (16 bits, compared by RFC 1982 serial arithmetic), its id, codec, sample-rate code, block size, sample count and block count, then optional extensions as tag, length and value: a label of at most 64 bytes (tag 1) and a BCP 47 language tag of at most 16 bytes (tag 2). Unknown tags are skipped. The block count shall equal the sample count divided by the samples of a whole block, rounded up, or the record is refused. A sample count of 0 marks a live sound (REQ-AUD-014).
*Verification:* Tests of a labelled object, a wrong block count and a reserved codec.

**REQ-AUD-005 — The DATA record and block geometry.**
A DATA record shall carry a handle, a generation, the index of its first block, and one or more whole blocks. An IMA ADPCM block is 256 or 512 bytes and holds 1 + 2 × (bytes − 4) samples; a PCM block is an even number of bytes, at most 1 000. Only a sound's final block may be shorter, and then only as long as its samples need. Blocks of the wrong length or beyond the sound's end make the record discarded. *Amended 26 October 2026 (decision E.124):* an Opus block is one packet of the object's block size, 1 to 160 bytes, the same for every block of the sound, and holds 960 samples; its final block is whole.
*Verification:* Store tests of whole, final and malformed blocks; Opus sounds carried end to end (`tests/receiver.rs`, module `opus`).

**REQ-AUD-006 — The CONTROL record.**
A CONTROL record shall carry, in 14 bytes: the channel (0 to 3), a sequence number, the operation (1 play, 2 stop, 3 set gain), the sound's handle and generation, a gain of 0 to 100 per cent, a fade of at most 10 000 milliseconds, flags (1 progressive, 2 loop, 4 after current; others zero), a duck code of 0 to 6, and a page number of 100 to 899 in EN 300 706 form, or 0xFFFF for the whole service.
*Verification:* Tests of every field's bounds.

**REQ-AUD-007 — Broadcast carriage.**
Records shall be carried on independent data lines of EN 300 708 clause 6.7.3, data channel 12 (packet 4/31), as a user-defined service of Table 21: Service Byte message bit 4 set, bits 1 to 3 naming one of eight audio services. The Control Byte, Hamming 8/4 coded, shall carry bit 1 for a record's first fragment and bit 2 for its last; bits 3 and 4 are zero. Bytes 8 to 45 shall hold a record id (counting new records modulo 256, not repeats), the fragment index (0 to 30), 34 bytes of the record, and a 16-bit check over the 36 bytes before it with the polynomial of EN 300 708 clause 6.5.8.1. A receiver shall drop a packet that fails any of these, reassemble a record from its fragments in any order, and hold at most two records in flight (eight on a desktop), replacing the oldest incomplete one. The teletext decoder itself keeps ignoring packet 31 (REQ-PKT-008); the port passes packets to the audio receiver first.
*Verification:* Tests that data channel 12 decodes as packet 4/31; reassembly out of order, with a lost fragment repaired by a repeat, and with replacement; dropped packets; the fuzz target `audio_packets`.

**REQ-AUD-008 — Network and file carriage.**
Over a WebSocket, records shall travel as the broadcast carriage's packets, whole 42-byte packets in binary messages, as REQ-WEB-005 defines them: a binary message is packets, never a bare record (*amended 20 October 2026, decision E.92*: the first text, one record a binary message, conflicted with REQ-WEB-005). A `.ttau` file shall be records back to back; a record whose length breaks a bound ends the file, and a truncated final record is discarded. A record never names its own service: the host associates each connection or file with one.
*Amended 28 October 2026 (decision E.131):* both readers are built: `Receiver::push_file` reads a `.ttau` file, and `Receiver::push_message` a WebSocket's binary message, refusing whole a message that is not whole packets.
*Verification:* Tests of a file's records, of a truncated final record and of a length beyond the bound (`ttau` module; `a_ttau_file_is_read_as_records_back_to_back`), and of a message of packets and a message refused whole (`a_websocket_message_is_packets_and_a_ragged_one_is_refused_whole`); the fuzz targets `audio_file` and `audio_messages`.

**REQ-AUD-009 — The object store.**
A receiver shall keep sounds compressed in a fixed arena, admitting a sound only if it will fit once complete. It shall evict the oldest sound no channel has armed or waiting, and report a resource limit when nothing can be evicted. A repeated OBJECT changes nothing; a newer generation replaces the older and tells the sink to forget it; an older generation, or a different definition of the same generation, is a conflict and is refused. A block that arrives again with different bytes puts the sound in conflict, and a sound in conflict is not played until a newer generation replaces it.
*Verification:* Store tests of storage, completion, conflicts, generations, eviction and the resource limit.

**REQ-AUD-010 — The latest command wins.**
For each channel a receiver shall keep only the sequence number of the last command it accepted, and accept a command only if its sequence number is newer by RFC 1982 serial arithmetic over 16 bits. A repeated command, or one that arrives after a newer one, changes nothing. A change of service clears every channel.
*Verification:* Tests of repeats, late arrivals, a stop arriving before the older play it cancels, the wrap and the half-range case.

**REQ-AUD-011 — Operations.**
A play shall start its sound on the channel, ramping out what the channel played; with *after current* it waits for that sound to end, a channel holding one such waiting sound. With *loop* it repeats until a later command stops it. A stop ramps the channel to silence and drops its waiting sound. Set gain ramps the channel to the new gain. A play whose sound has not arrived, or is incomplete, waits until it is ready.
*Verification:* Receiver tests of waiting plays, and mixer tests of replacement, queueing and stopping.

**REQ-AUD-012 — Sounds that belong to a page.**
A play naming a page shall be armed for that page. It shall start whenever that page is opened, and fade out when the page is left; a repeat of the record arms nothing new. A page sound shall play only while the user's *page sounds* setting is on. A play for the whole service plays once, when it is accepted. Receiving data never starts a sound by itself.
*Verification:* Receiver tests of opening, leaving and reopening a page, and of the setting.

**REQ-AUD-013 — Starting and loss.**
A play shall start once its whole sound has arrived, or, with the *progressive* flag, once 40 milliseconds of it have arrived contiguously from the start (or all of it, if shorter); later blocks are passed to the sink as they arrive. Samples of blocks that have not arrived are silence at the sink, and the sound's timing is never shifted or stretched.
*Amended 28 October 2026 (decision E.131):* the silence of a block that has not arrived shall be faded into and out of over 5 milliseconds, on the samples either side of it, and a stored block's faded edge sent again whole once the block next to it arrives. Each block given as silence, or concealed by its decoder, shall be reported `concealed` to the sink (`VoiceSink::concealed`) and counted: a live block when it is given up, and a stored sound's block when, by the receiver's clock from the play's start, the play reaches it before it is at the sink, once a play. A play that waits for the sound before it, and a loop after its first pass, are not followed.
*Verification:* A receiver test of a progressive start and of later blocks reaching the sink; receiver tests of a stored block's faded edges and their mending (`a_missing_stored_block_is_faded_either_side_and_mended_when_it_comes`), of a live block given up (`a_lost_live_block_is_faded_into_and_out_of_and_reported_concealed`), of a stored Opus sound's decoded samples ending faded (`a_stored_opus_sound_fades_where_its_blocks_stop_and_mends_when_they_come`) and of a progressive play's report (`a_progressive_play_reports_each_block_not_there_when_it_is_due`); unit tests of the ramp.

**REQ-AUD-025 — Opus decoded in order.**
An Opus sound's packets shall be decoded in index order, by one decoder that follows the sound: a stored sound from its first block, as far as its blocks have arrived without a gap; a live sound from where it joined, a lost packet concealed. The receiver shall keep at most `OPUS_DECODERS` decoders; a stored sound frees its decoder when its last block is decoded. A packet the decoder refuses is concealed and counted as a decode failure. The engine carries an encoder's packets and encodes no Opus itself.
*Verification:* The sink's samples equal one decoder's output for the same packets, for a stored sound arriving in reverse order, a progressive play and a live sound with a lost packet (`tests/receiver.rs`, module `opus`).

**REQ-AUD-014 — Live sounds.**
A sound with a sample count of 0 is live: its blocks arrive without end, each whole, their index counting modulo 2¹⁶ (RFC 1982). A receiver shall:
- **Hold** the newest 32 blocks in a ring, dropping a block older than the ring;
- **Join** at the newest block when a play can start, and give the sink each later block in index order; a block still missing once three newer ones have arrived is given as silence and counted as `concealed` (an Opus block is concealed by its decoder instead, decision E.124);
- **Follow** the sender's clock: the sink buffers the stream and trims its playing rate to hold the buffer level, absorbing at least ±1 per cent of clock error (the mixer: 400 ms buffered, ±5 per cent);
- **Fade out** over 100 ms when no new block has come for two seconds, as EN 300 708 clause 6.7.1.1 asks, forget the sound at the sink, and start the play again, at the newest block, when blocks return.
The port tells the receiver the time (`Receiver::tick`), and the receiver says when it next needs it (`Receiver::next_tick_ms`): when a live sound at the sink would stall, or never while none plays.
*Amended 21 October 2026 (decision E.98):* the receiver says when it next needs a tick.
*Amended 28 October 2026 (decision E.131):* the next tick is also the moment a followed play reaches a stored block that is not at the sink, for the `concealed` report of REQ-AUD-013; and a live PCM16 or IMA ADPCM block's last 5 milliseconds reach the sink once the next block arrives or is given up, so that they can be faded out before a silence.
*Verification:* Store tests of the ring and of the index wrapping; receiver tests of joining, of the next tick's time, ordered delivery, a late block, a lost block, the stall and the resumption, and of a stopped live sound staying stopped; mixer tests of priming, a sender 1 per cent fast and 1 per cent slow, playing out after the sound is forgotten, and skipping ahead when a ring behind; a `neottx sound --live` test played back at the browser's pace.

**REQ-AUD-015 — Gain, fades and ducking.**
Gain shall be a whole percentage. No level shall change faster than over 5 milliseconds: every start, stop, gain change and replacement ramps over the command's fade or 5 milliseconds, whichever is longer. A play's duck code shall lower every other channel, while it plays, to the attenuation of EN 300 708 Table 19 (0, 4, 8, 12, 18, 24 or 30 dB), applied as the nearest whole percentage (100, 63, 40, 25, 13, 6 or 3).
*Verification:* Mixer tests of the ramp's length, the stop, and a 12 dB duck.

**REQ-AUD-016 — Fewer voices than channels.**
A receiver whose sink plays fewer than four channels shall play the lowest-numbered ones and ignore commands for the rest.
*Verification:* A receiver test with a two-channel sink.

**REQ-AUD-017 — Receiver limits.**
A port shall size the store, the objects held and the records in flight, and publish them. A microcontroller receiver (Level A) holds at least 16 KB of sounds, 8 objects and 2 records in flight; a desktop receiver (Level B) at least 4 MB, 256 objects and 8 records.
*Verification:* Inspection of each port's constants.

**REQ-AUD-018 — The voice sink.**
The receiver shall drive a voice sink the port supplies, through `neoteletext_audio::port::VoiceSink`: define a sound, give it 16-bit samples, forget it, and start, stop or ramp a channel. The receiver decides what plays and when; the sink only plays. *Not yet built:* passing IMA ADPCM blocks undecoded to a sink that decodes them itself, such as tellyboy's.
*Amended 28 October 2026 (decision E.131):* the sink is also told of each block concealed (`VoiceSink::concealed`), for diagnostics; a sink may leave the method as provided, doing nothing.
*Verification:* The receiver's tests run through a recording sink.

**REQ-AUD-019 — The software mixer.**
A port with an operating system's audio output shall be able to use `neoteletext-audio-mix`: four voices mixed in integers, every change ramped (REQ-AUD-015), ducking, a clamp after the sum, a master volume and mute, and each voice resampled to the output rate by a windowed-sinc filter. Its rendering shall never allocate or free. A port that holds a lock its audio callback also takes shall be able to allocate a sound's samples, or a live sound's ring, before taking the lock and give the buffer to the mixer, which keeps it uncopied (`Mixer::define_with`, `Mixer::stream_with`, and `Mixer::stream_ring` for a ring's length); a ring of any other length is handed back.
*Amended 24 October 2026 (finding F-55 of NeoTeletext's security review):* the buffer-taking forms are added, so that NeoTeletext's sound helper no longer allocates a sound under the lock its device's audio thread takes.
*Verification:* Mixer tests of ramps, stops, clamping, ducking, queueing, mute, and a resampled tone's pitch and amplitude; that a sound given its buffer, and a live sound given its ring, mix exactly as ones the mixer allocated, that a given buffer is held uncopied, and that a ring of the wrong length is handed back with the mixer unchanged.

**REQ-AUD-020 — Writing sounds.**
The `encode` feature shall write sounds for publishing tools: IMA ADPCM blocks whose headers carry the encoder's state, OBJECT, DATA and CONTROL records, and records cut into the packets of REQ-AUD-007. `neottx sound` uses it (REQ-CLI-011).
*Verification:* The receiver's end-to-end tests read back what the encoder wrote.

**REQ-AUD-021 — Security.**
Every audio input is untrusted (area SEC). The parsers shall bound every length before using it, contain no panicking constructs, and be fuzzed. A label is shown only after the cleaning of page text. No sound starts without a command, and a page sound needs the user's setting (REQ-AUD-012).
*Verification:* The crates' no-panic lints; the fuzz targets `audio_records`, `audio_packets`, `audio_file` and `audio_messages`.

**REQ-AUD-022 — Opus packets.**
The Opus decoder (`neoteletext-opus`, decision E.123) shall take packets of one profile only: Opus mono, one 20 ms frame a packet, either bare (RFC 6716 §3.2.2, TOC code 0) or padded in a code 3 packet whose frame count is 1 (§3.2.5), at most 160 bytes. Any other packet (empty, longer, stereo, of several frames, of another duration, or with a frame count of 0 or padding beyond the packet) shall be refused with its reason before the decoder's state changes. A frame of one byte or none is one the encoder left out (§3.2.1) and shall be concealed.
*Reason:* The owner's profile (decision E.123); refusing before any change keeps a hostile packet from disturbing what follows.
*Verification:* Unit tests of each refusal and of the padded form; the fuzz target `opus_packets`.

**REQ-AUD-023 — Opus decoding, bit-exact with the reference.**
For every sequence of profile packets and losses, the decoder shall produce, frame for frame, the same 960 samples at 48 kHz and the same final range as libopus 1.5.2's fixed-point decoder given the same packets, a loss passed to it as no packet and in-band FEC not decoded (decision E.123). A frame shall never depend on what the caller's buffer held before.
*Reason:* An integer decoder proved equal to the reference implementation of RFC 6716 needs no tolerance and no listening test to show it correct, on any processor.
*Verification:* `tests/oracles/run_opus.sh` in the full gate: libopus, downloaded and pinned by checksum, encodes synthetic sources and any in `$NEOTELETEXT_CORPUS/opus-sources/` under every setting that changes the bitstream, and its decoder and this one decode every stream as received and with losses, compared byte for byte; the unit test that a frame does not depend on the buffer's old contents.

**REQ-AUD-024 — The Opus decoder on a small processor.**
The decoder shall be `no_std`, allocate nothing, depend on no crate and use no floating point; its state shall be one value of fixed size and its scratch held in that state; it shall link no panic path. Its flash, state, stack and instructions a frame on the Cortex-M0 shall be measured and recorded (decision E.123).
*Reason:* The owner's order is desktops first and microcontrollers later, with one decoder for both; the tellyboy targets have tens of kilobytes of RAM and no floating-point unit.
*Verification:* The `opus` build of `cargo xtask size` (flash and the symbol check of REQ-GEN-019); `cargo xtask check-deps`; the measurements of decision E.123.
