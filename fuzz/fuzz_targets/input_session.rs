//! A resumable input session driven by an arbitrary channel (REQ-RCH-013,
//! REQ-INT-106, REQ-INT-110, REQ-TST-040): acknowledgements, WELCOMEs that
//! resume from any count, lost connections and the receiver's input in any
//! order never panic; what it sends again is whole messages it sent, in
//! order, and only after a resumed WELCOME; and the pacer of the pointer's
//! moves (REQ-RCH-031 to REQ-RCH-033) never sends two moves of a contact
//! less than a period apart unless flushed.
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_input::caps::{RESUME_CAP, SELECT_CAP};
use neoteletext_input::control::Resumption;
use neoteletext_input::event::{Device, Event, Item, Mouse, MouseAction, Point, Stamp, read_message};
use neoteletext_input::pace::{Offer, PERIOD_WHOLE_US, Pacer};
use neoteletext_input::session::{ResumableSession, Welcomed};
use neoteletext_input::{MAX_MESSAGE, TOKEN_BYTES};

fuzz_target!(|data: &[u8]| {
    let mut session = ResumableSession::new(SELECT_CAP);
    let mut pacer = Pacer::new();
    let mut now_ms = 0_u32;
    let mut now_us = 0_u64;
    let mut sent: Vec<Vec<u8>> = Vec::new();
    let mut last_move_at: Option<u64> = None;
    let mut out = [0_u8; MAX_MESSAGE];
    let mut bytes = data.iter().copied();
    while let Some(op) = bytes.next() {
        let value = u16::from_be_bytes([bytes.next().unwrap_or(0), bytes.next().unwrap_or(0)]);
        now_ms = now_ms.wrapping_add(u32::from(op >> 4));
        now_us = now_us.saturating_add(u64::from(value) % 40_000);
        match op & 0x0F {
            0 => {
                let length = session.open(now_ms, None, &mut out).expect("the first message fits");
                read_message(&out[..length], &mut |_| {}).expect("it reads");
            }
            1 => session.close(),
            2 => {
                let taken = (value & 1 == 1).then_some(value >> 1);
                let resume = Some(Resumption { token: [op; TOKEN_BYTES], taken });
                if let Ok(Welcomed::Resumed) = session.welcome(neoteletext_input::VERSION, SELECT_CAP | RESUME_CAP, resume) {
                    let mut again = Vec::new();
                    session.resend(&mut |message| again.push(message.to_vec()));
                    // What goes again is the end of what was sent, whole messages in order.
                    assert!(again.len() <= sent.len());
                    assert_eq!(&sent[sent.len() - again.len()..], again.as_slice());
                }
            }
            3 => {
                session.acknowledged(value);
            }
            4 => {
                let text = ["a", "bc", "déf"][usize::from(value % 3)];
                if let Ok(length @ 1..) = session.send(&Item::Text(text), now_ms, &mut out) {
                    sent.push(out[..length].to_vec());
                }
            }
            5 => session.forget(),
            _ => {
                let at = Point { column: u32::from(value % 40), row: 1, pixels: Some((u32::from(value), 0)), mods: 0 };
                let on = Stamp { page: 0x100, sub: 0 };
                let moved = Event::Mouse(Mouse { on, action: MouseAction::Move, button: 0, at, device: Device::Mouse });
                // A move that waited went at the end of its period, not when
                // the timer ran: the pace is counted from there.
                let due = pacer.due();
                pacer.take_due(now_us, &mut |_| last_move_at = due);
                match pacer.offer(moved, now_us) {
                    Offer::Now(_) => {
                        if let Some(before) = last_move_at {
                            assert!(now_us - before >= PERIOD_WHOLE_US, "a move within a period of the last");
                        }
                        last_move_at = Some(now_us);
                    }
                    Offer::Held { due_us } => assert!(due_us > now_us),
                    Offer::Unchanged | Offer::NotAMove => {}
                }
                if op & 0x80 != 0 {
                    pacer.flush(now_us, &mut |_| {});
                    last_move_at = None;
                }
            }
        }
    }
});
