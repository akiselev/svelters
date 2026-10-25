//! The caret blink rule: on for the first half of each cycle from the last caret change, steady after
//! the timeout; and the engine restarting it on every `FastCommit::Caret`.

use presto_base::time::{Span, Time};
use presto_compositor::caret::CaretBlink;
use std::time::Duration;

const MS: i64 = 1_000_000;

#[test]
fn visible_for_the_first_half_of_each_cycle() {
    let b = CaretBlink { cycle: Duration::from_millis(1000), timeout: None };
    assert!(b.visible(Span(0)));
    assert!(b.visible(Span(499 * MS)));
    assert!(!b.visible(Span(500 * MS)));
    assert!(!b.visible(Span(999 * MS)));
    assert!(b.visible(Span(1000 * MS)));
    assert!(!b.visible(Span(10_700 * MS)));
}

#[test]
fn steady_after_the_timeout() {
    let b = CaretBlink::GTK;
    assert!(!b.visible(Span(600 * MS)));
    assert!(b.visible(Span(10_000 * MS)));
    assert!(b.visible(Span(10_600 * MS)), "no more blinking after 10 s");
    let since = Time(5_000 * MS as u64);
    assert_eq!(b.next_change(since, since), Some(since + Span(600 * MS)));
    assert_eq!(b.next_change(since, since + Span(9_900 * MS)), Some(since + Span(10_000 * MS)));
    assert_eq!(b.next_change(since, since + Span(10_000 * MS)), None);
}

#[test]
fn next_change_is_the_next_half_cycle_boundary() {
    let b = CaretBlink::WINDOWS;
    let since = Time(1_000 * MS as u64);
    assert_eq!(b.next_change(since, since + Span(100 * MS)), Some(since + Span(530 * MS)));
    assert_eq!(b.next_change(since, since + Span(530 * MS)), Some(since + Span(1060 * MS)));
}

#[test]
fn defaults_per_os() {
    assert_eq!(CaretBlink::GTK.cycle, Duration::from_millis(1200));
    assert_eq!(CaretBlink::QT.cycle, Duration::from_millis(1000));
    assert_eq!(CaretBlink::WINDOWS.cycle, Duration::from_millis(1060));
    assert_eq!(CaretBlink::MACOS.cycle, Duration::from_millis(1060));
}
