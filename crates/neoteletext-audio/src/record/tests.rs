#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::arithmetic_side_effects, clippy::panic)]

// Tests of REQ-AUD-002, REQ-AUD-003, REQ-AUD-004 and REQ-AUD-006.
use super::*;
use crate::crc::crc32;
use crate::{MAX_BODY, PROFILE_VERSION};
use std::vec::Vec;

fn frame(kind: u8, body: &[u8]) -> Vec<u8> {
    let mut bytes = std::vec![PROFILE_VERSION, kind];
    bytes.extend_from_slice(&u16::try_from(body.len()).unwrap().to_be_bytes());
    bytes.extend_from_slice(body);
    let crc = crc32(&bytes);
    bytes.extend_from_slice(&crc.to_be_bytes());
    bytes
}

fn object_body() -> Vec<u8> {
    // Handle 3, generation 1, id 9, IMA ADPCM, 16 kHz, 256-byte blocks,
    // 1 000 samples in 2 blocks of 505.
    let mut body = std::vec![3, 0, 1, 0, 0, 0, 9, 1, 2, 1, 0];
    body.extend_from_slice(&1000_u32.to_be_bytes());
    body.extend_from_slice(&2_u16.to_be_bytes());
    body
}

#[cfg(feature = "ima")]
#[test]
fn an_object_with_a_label_is_read() {
    let mut body = object_body();
    body.extend_from_slice(&[TAG_LABEL, 5]);
    body.extend_from_slice(b"Chime");
    body.extend_from_slice(&[0x7F, 1, 0]); // an unknown tag, skipped
    let bytes = frame(TYPE_OBJECT, &body);
    let Record::Object(object) = parse(&bytes).unwrap() else { panic!("not an object") };
    assert_eq!(object.codec, Some(Codec::ImaAdpcm));
    assert_eq!((object.rate_hz, object.samples, object.blocks), (16_000, 1000, 2));
    assert_eq!(object.label, Some(&b"Chime"[..]));
}

#[cfg(feature = "ima")]
#[test]
fn a_wrong_block_count_is_refused() {
    let mut body = object_body();
    let last = body.len() - 1;
    body[last] = 3;
    assert_eq!(parse(&frame(TYPE_OBJECT, &body)), Err(RecordError::BadValue("block count")));
}

#[test]
fn a_reserved_codec_is_read_and_left_for_the_receiver_to_ignore() {
    let mut body = object_body();
    body[7] = 3;
    let bytes = frame(TYPE_OBJECT, &body);
    let Record::Object(object) = parse(&bytes).unwrap() else { panic!("not an object") };
    assert_eq!(object.codec, None);
}

/// Decision E.131: a build without `ima` or `pcm16` reads their objects as
/// of no codec it decodes, so the receiver ignores them as unsupported.
#[test]
fn codecs_left_out_of_the_build_are_read_as_unsupported() {
    let ima = frame(TYPE_OBJECT, &object_body());
    let Record::Object(object) = parse(&ima).unwrap() else { panic!("not an object") };
    assert_eq!(object.codec.is_some(), cfg!(feature = "ima"));
    let mut pcm = object_body();
    // PCM, 100-byte blocks: 1 000 samples in 20 blocks of 50.
    pcm[7] = 0;
    pcm[9..11].copy_from_slice(&100_u16.to_be_bytes());
    let last = pcm.len() - 2;
    pcm[last..].copy_from_slice(&20_u16.to_be_bytes());
    let pcm = frame(TYPE_OBJECT, &pcm);
    let Record::Object(object) = parse(&pcm).unwrap() else { panic!("not an object") };
    assert_eq!(object.codec.is_some(), cfg!(feature = "pcm16"));
}

/// An Opus object: handle 3, generation 1, id 9, codec 2, rate code `rate`,
/// packets of `packet` bytes, `samples` samples in `blocks` blocks.
fn opus_body(rate: u8, packet: u16, samples: u32, blocks: u16) -> Vec<u8> {
    let mut body = std::vec![3, 0, 1, 0, 0, 0, 9, 2, rate];
    body.extend_from_slice(&packet.to_be_bytes());
    body.extend_from_slice(&samples.to_be_bytes());
    body.extend_from_slice(&blocks.to_be_bytes());
    body
}

/// Decision E.124: code 2 is Opus, one 960-sample packet a block, at 48 kHz
/// only, packets of 1 to 160 bytes.
#[cfg(feature = "opus")]
#[test]
fn opus_objects_are_read_at_48_khz_with_packets_the_decoder_takes() {
    const RATE_48_KHZ: u8 = 7;
    const RATE_16_KHZ: u8 = 2;
    let bytes = frame(TYPE_OBJECT, &opus_body(RATE_48_KHZ, 40, 2000, 3));
    let Record::Object(object) = parse(&bytes).unwrap() else { panic!("not an object") };
    assert_eq!(object.codec, Some(Codec::Opus));
    assert_eq!((object.rate_hz, object.block_bytes, object.blocks), (48_000, 40, 3));
    assert_eq!(
        parse(&frame(TYPE_OBJECT, &opus_body(RATE_16_KHZ, 40, 2000, 3))),
        Err(RecordError::BadValue("sample rate"))
    );
    assert_eq!(
        parse(&frame(TYPE_OBJECT, &opus_body(RATE_48_KHZ, 161, 2000, 3))),
        Err(RecordError::BadValue("block size"))
    );
    assert_eq!(
        parse(&frame(TYPE_OBJECT, &opus_body(RATE_48_KHZ, 40, 2000, 2))),
        Err(RecordError::BadValue("block count"))
    );
}

/// Built without the decoder, a receiver ignores Opus as a reserved code.
#[cfg(not(feature = "opus"))]
#[test]
fn without_the_decoder_opus_is_ignored() {
    let bytes = frame(TYPE_OBJECT, &opus_body(7, 40, 2000, 3));
    let Record::Object(object) = parse(&bytes).unwrap() else { panic!("not an object") };
    assert_eq!(object.codec, None);
}

#[test]
fn controls_are_read_and_their_bounds_kept() {
    let body = [1, 0, 7, 1, 3, 0, 1, 80, 0, 200, FLAG_LOOP, 2, 0x01, 0x00];
    let Record::Control(control) = parse(&frame(TYPE_CONTROL, &body)).unwrap() else { panic!("not a control") };
    assert_eq!(control.page, Some(0x100));
    assert_eq!((control.channel, control.sequence, control.gain, control.fade_ms), (1, 7, 80, 200));
    for (index, value, error) in [
        (0, 4, "channel"),
        (3, 9, "operation"),
        (7, 101, "gain"),
        (10, 8, "flags"),
        (11, 7, "duck"),
        (12, 0x09, "page"),
    ] {
        let mut bad = body;
        bad[index] = value;
        assert_eq!(parse(&frame(TYPE_CONTROL, &bad)), Err(RecordError::BadValue(error)), "field {index}");
    }
    // A hexadecimal page, and a page for the whole service.
    let mut bad = body;
    bad[13] = 0x0A;
    assert_eq!(parse(&frame(TYPE_CONTROL, &bad)), Err(RecordError::BadValue("page")));
    let mut service = body;
    service[12..14].copy_from_slice(&SERVICE_SCOPE.to_be_bytes());
    assert!(matches!(parse(&frame(TYPE_CONTROL, &service)), Ok(Record::Control(Control { page: None, .. }))));
}

#[test]
fn damaged_and_malformed_frames_are_refused() {
    let good = frame(TYPE_OBJECT, &object_body());
    assert!(parse(&good).is_ok());
    let mut damaged = good.clone();
    damaged[6] ^= 1;
    assert_eq!(parse(&damaged), Err(RecordError::Crc));
    assert_eq!(parse(&good[..good.len() - 1]), Err(RecordError::Truncated));
    let mut longer = good.clone();
    longer.push(0);
    assert_eq!(parse(&longer), Err(RecordError::TrailingBytes));
    assert_eq!(parse(&frame(9, &[])), Err(RecordError::UnknownType(9)));
    // Version 1 with a correct CRC.
    let mut version = std::vec![1, TYPE_CONTROL];
    version.extend_from_slice(&u16::try_from(CONTROL_BYTES).unwrap().to_be_bytes());
    version.extend_from_slice(&[0; CONTROL_BYTES]);
    let crc = crc32(&version);
    version.extend_from_slice(&crc.to_be_bytes());
    assert_eq!(parse(&version), Err(RecordError::Version(1)));
    let mut too_long = std::vec![PROFILE_VERSION, TYPE_DATA];
    too_long.extend_from_slice(&u16::try_from(MAX_BODY + 1).unwrap().to_be_bytes());
    assert_eq!(framed_length(&too_long), Err(RecordError::TooLong));
}
