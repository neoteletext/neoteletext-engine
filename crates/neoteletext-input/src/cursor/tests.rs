#![allow(clippy::indexing_slicing, clippy::unwrap_used)]

// Tests of REQ-INT-005 and REQ-INT-006.
use super::*;
use neoteletext_audio::record::TYPE_CONTROL;

fn cursor() -> Cursor {
    Cursor { page: 0x100, acked: 7, place: Some(Place { row: 5, column: 10 }), flags: BLOCK }
}

// REQ-INT-005 and REQ-INT-110.
#[test]
fn a_cursor_round_trips_through_a_packet() {
    let packet = cursor().packet(3, 200).unwrap();
    assert_eq!(from_packet(&packet), Some(Ok(cursor())));
    let address = neoteletext_core::packet::decode_address(&packet).unwrap();
    assert_eq!((address.magazine.number(), address.number), (4, 31));
    assert_eq!(RECORD, 16);
}

#[test]
fn no_cursor_carries_the_acknowledgement_alone() {
    let game = Cursor { page: 0x8FF, acked: 65_535, place: None, flags: 0 };
    assert_eq!(read(&game.record().unwrap()), Ok(game));
}

#[test]
fn every_field_is_checked() {
    let refused = |change: fn(&mut Cursor)| {
        let mut cursor = cursor();
        change(&mut cursor);
        let mut out = [0; RECORD];
        write(TYPE_CURSOR, &cursor.body(), &mut out).unwrap();
        assert!(read(&out).is_err(), "{cursor:?}");
        assert!(cursor.record().is_none());
    };
    refused(|cursor| cursor.page = 0x0FF);
    refused(|cursor| cursor.page = 0x900);
    refused(|cursor| cursor.place = Some(Place { row: 0, column: 0 }));
    refused(|cursor| cursor.place = Some(Place { row: 24, column: 0 }));
    refused(|cursor| cursor.place = Some(Place { row: 1, column: 40 }));
    refused(|cursor| cursor.flags = 8);
    refused(|cursor| {
        cursor.place = None;
    });
}

/// Decision E.134: the echo width and the echo and masked flags are
/// withdrawn. A channel written before still sets them; its cursor is read
/// all the same, without them, and nothing writes them.
#[test]
fn the_withdrawn_echo_fields_are_ignored() {
    let mut body = cursor().body();
    assert_eq!(body[6], 0, "the reserved byte is written 0");
    body[6] = 20;
    body[7] |= 1 | 2;
    assert_eq!(from_body(body), Ok(cursor()));
    let mut none = Cursor { page: 0x100, acked: 1, place: None, flags: 0 }.body();
    none[6] = 9;
    assert_eq!(from_body(none).map(|cursor| cursor.place), Ok(None));
}

#[test]
fn other_records_and_damage_are_not_cursors() {
    let mut out = [0; RECORD];
    write(TYPE_CONTROL, &cursor().body(), &mut out).unwrap();
    assert_eq!(read(&out), Err(CursorError::NotCursor(TYPE_CONTROL)));
    let mut record = cursor().record().unwrap();
    record[6] ^= 1;
    assert!(matches!(read(&record), Err(CursorError::Frame(RecordError::Crc))));
}

#[test]
fn a_sound_on_the_same_channel_is_not_a_cursor() {
    // A CONTROL record is one fragment too.
    let body = [0_u8; 14];
    let mut out = [0; 22];
    let length = write(TYPE_CONTROL, &body, &mut out).unwrap();
    let packet = packet(0, 1, 0, true, &out[..length]);
    assert_eq!(from_packet(&packet), None);
}
