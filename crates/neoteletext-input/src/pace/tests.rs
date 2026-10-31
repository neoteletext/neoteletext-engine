//! Tests of the pacer (REQ-INT-122; REQ-RCH-031 to REQ-RCH-033, REQ-RCH-045).
#![allow(clippy::unwrap_used, clippy::arithmetic_side_effects, clippy::indexing_slicing, reason = "a test")]

use super::*;
use crate::event::{Device, Mouse, MouseAction, Point, Stamp, Touch};
use std::vec::Vec;

const MS: u64 = 1_000;
/// The page on screen in these tests.
const ON: Stamp = Stamp { page: 0x100, sub: 0 };

fn mouse(column: u32) -> Event<'static> {
    let at = Point { column, row: 5, pixels: Some((column * 12, 100)), mods: 0 };
    Event::Mouse(Mouse { on: ON, action: MouseAction::Move, button: 0, at, device: Device::Mouse })
}

fn touch(id: u8, column: u32) -> Event<'static> {
    Event::Touch(Touch { on: ON, action: MouseAction::Move, id, column, row: 5, pixels: None })
}

/// REQ-INT-122, REQ-RCH-031: the period is 1000/60 ms, counted exactly.
#[test]
fn the_period_is_a_sixtieth_of_a_second() {
    assert_eq!(MOVES_PER_SECOND, 60);
    assert_eq!(PERIOD_WHOLE_US, 16_666);
    assert_eq!(PERIOD_WHOLE_US * MOVES_PER_SECOND + PERIOD_REMAINDER, 1_000_000);
}

/// REQ-INT-122, REQ-RCH-032: a move 20 ms after the last goes at once; one 5 ms after
/// it waits until 16⅔ ms after the last, and goes carrying the latest
/// position; no timer is needed while the slot is empty.
#[test]
fn a_move_goes_at_once_if_it_can_and_the_latest_is_kept() {
    let mut pacer = Pacer::new();
    assert_eq!(pacer.offer(mouse(1), 0), Offer::Now(mouse(1)));
    assert_eq!(pacer.due(), None, "nothing waits: no timer");
    assert_eq!(pacer.offer(mouse(2), 20 * MS), Offer::Now(mouse(2)), "20 ms after: at once");
    assert_eq!(pacer.offer(mouse(3), 25 * MS), Offer::Held { due_us: 20 * MS + 16_666 });
    assert_eq!(pacer.offer(mouse(4), 30 * MS), Offer::Held { due_us: 20 * MS + 16_666 }, "replaces it");
    assert_eq!(pacer.due(), Some(36_666));
    let mut sent = Vec::new();
    pacer.take_due(36_665, &mut |event| sent.push(event));
    assert!(sent.is_empty(), "not before its time");
    pacer.take_due(36_666, &mut |event| sent.push(event));
    assert_eq!(sent, [mouse(4)], "the latest");
    assert_eq!(pacer.due(), None);
    assert_eq!(pacer.offer(mouse(4), 60 * MS), Offer::Unchanged, "an unchanged position does not go again");
    assert_eq!(pacer.offer(mouse(5), 40 * MS), Offer::Held { due_us: 53_333 });
    assert_eq!(pacer.offer(mouse(4), 41 * MS), Offer::Unchanged, "back where it was: nothing waits");
    assert_eq!(pacer.due(), None);
}

/// REQ-INT-122, REQ-RCH-031: a thousand moves a second, for ten seconds: sixty a
/// second, the 600th period ending exactly at ten seconds.
#[test]
fn a_thousand_moves_a_second_go_sixty_with_no_drift() {
    let mut pacer = Pacer::new();
    let mut sent_at = Vec::new();
    for millisecond in 0..10_000_u32 {
        let now = u64::from(millisecond) * MS;
        pacer.take_due(now, &mut |_| sent_at.push(now));
        let at = Point { column: 1, row: 5, pixels: Some((millisecond, 0)), mods: 0 };
        let moved = Event::Mouse(Mouse { on: ON, action: MouseAction::Move, button: 0, at, device: Device::Mouse });
        if let Offer::Now(_) = pacer.offer(moved, now) {
            sent_at.push(now);
        }
    }
    assert_eq!(sent_at.len(), 600, "60 a second");
    for (k, at) in sent_at.iter().enumerate() {
        let deadline = k as u64 * 1_000_000 / MOVES_PER_SECOND;
        assert!(*at >= deadline && *at < deadline + MS, "move {k} at {at}, its period ends at {deadline}");
    }
    assert_eq!(pacer.due(), Some(10_000_000), "the next period ends at exactly 10 s");
}

/// REQ-INT-122, REQ-RCH-033: before any other event, a waiting move is flushed, and it
/// begins the next run.
#[test]
fn a_waiting_move_is_flushed_before_any_other_event() {
    let mut pacer = Pacer::new();
    pacer.offer(mouse(1), 0);
    assert!(matches!(pacer.offer(mouse(2), 5 * MS), Offer::Held { .. }));
    let mut flushed = Vec::new();
    pacer.flush(6 * MS, &mut |event| flushed.push(event));
    assert_eq!(flushed, [mouse(2)], "the drag's last move, then the release");
    assert_eq!(pacer.due(), None);
    assert_eq!(pacer.offer(mouse(3), 10 * MS), Offer::Held { due_us: 6 * MS + 16_666 }, "paced from the flush");
    pacer.forget(0);
    assert_eq!(pacer.offer(mouse(3), 11 * MS), Offer::Now(mouse(3)), "a contact forgotten starts afresh");
}

/// REQ-INT-122, REQ-RCH-045: each contact has its own slot and pace, and the moves of
/// several due at one moment come together.
#[test]
fn each_contact_is_paced_alone_and_due_moves_come_together() {
    let mut pacer = Pacer::new();
    assert_eq!(pacer.offer(mouse(1), 0), Offer::Now(mouse(1)));
    assert_eq!(pacer.offer(touch(3, 1), MS), Offer::Now(touch(3, 1)), "another contact: its own pace");
    assert!(matches!(pacer.offer(mouse(2), 2 * MS), Offer::Held { .. }));
    assert!(matches!(pacer.offer(touch(3, 2), 3 * MS), Offer::Held { .. }));
    let mut together = Vec::new();
    pacer.take_due(20 * MS, &mut |event| together.push(event));
    assert_eq!(together, [mouse(2), touch(3, 2)], "both at one moment, to be written together");
    assert_eq!(pacer.offer(touch(MAX_CONTACTS as u8, 1), 0), Offer::NotAMove, "no such contact");
    let press = Event::Touch(Touch { on: ON, action: MouseAction::Press, id: 1, column: 1, row: 1, pixels: None });
    assert_eq!(pacer.offer(press, 0), Offer::NotAMove, "only moves");
}
