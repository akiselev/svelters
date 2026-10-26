//! The latency recorder's invariant (every seq terminates exactly once), its histograms and trace, and
//! the input coalescer.

use skelter::coalesce::{Coalescer, Pushed};
use skelter::latency::{
    DropReason, Histogram, InputClass, LatencyConfig, LatencyRecorder, Segment, Stage, Termination,
    Violation,
};
use skelter_base::event::{
    GesturePhase, InputEnvelope, InputEvent, KeyEvent, KeyLocation, KeyState, Modifiers,
    MouseButton, PointerEvent, PointerPhase, PointerType, ScrollDelta, WheelEvent,
};
use skelter_base::geom::{Point, Vec2};
use skelter_base::ids::{FrameId, InputSeq, Revision, WindowId};
use skelter_base::key::Key;
use skelter_base::smallvec::SmallVec;
use skelter_base::time::{Span, Time};

const W: WindowId = WindowId(1);

fn ms(n: u64) -> Time {
    Time(n * 1_000_000)
}

fn key(seq: u64, t: Time) -> InputEnvelope {
    InputEnvelope {
        seq: InputSeq(seq),
        window: W,
        os_time: t,
        recv_time: t + Span::from_micros(500),
        event: InputEvent::Key(KeyEvent {
            key: Some(Key::A),
            physical: 0,
            text: Some("a".into()),
            state: KeyState::Pressed,
            repeat: false,
            location: KeyLocation::Standard,
            modifiers: Modifiers::default(),
        }),
    }
}

fn pointer(seq: u64, t: Time, phase: PointerPhase, x: f32, buttons: u32) -> InputEnvelope {
    InputEnvelope {
        seq: InputSeq(seq),
        window: W,
        os_time: t,
        recv_time: t,
        event: InputEvent::Pointer(PointerEvent {
            phase,
            pointer: 0,
            kind: PointerType::Mouse,
            button: matches!(phase, PointerPhase::Down | PointerPhase::Up)
                .then_some(MouseButton::Primary),
            buttons,
            pos: Point::new(x, 0.0),
            modifiers: Modifiers::default(),
            pressure: 0.0,
            tilt: Vec2::ZERO,
            history: SmallVec::new(),
        }),
    }
}

fn wheel(seq: u64, dy: f32, phase: Option<GesturePhase>) -> InputEnvelope {
    InputEnvelope {
        seq: InputSeq(seq),
        window: W,
        os_time: ms(seq),
        recv_time: ms(seq),
        event: InputEvent::Wheel(WheelEvent {
            pos: Point::ZERO,
            delta: ScrollDelta::Pixels(Vec2::new(0.0, dy)),
            phase,
            momentum: None,
            modifiers: Modifiers::default(),
        }),
    }
}

/// Drive one key through every stage of the pipeline.
fn full_pipeline(r: &mut LatencyRecorder, seq: u64, t0: Time) {
    r.begin(&key(seq, t0));
    r.stamp(InputSeq(seq), Stage::DispatchEnd, t0 + Span::from_millis(1));
    r.stamp(InputSeq(seq), Stage::JsEnd, t0 + Span::from_millis(2));
    r.stamp_pending(
        W,
        Stage::DispatchEnd,
        Stage::UiFrameStart,
        t0 + Span::from_millis(3),
    );
    let sent = r.commit_sent(W, Revision(seq), t0 + Span::from_millis(5));
    assert_eq!(sent, vec![InputSeq(seq)]);
    r.commit_applied(W, Revision(seq), t0 + Span::from_millis(6));
    r.frame_stages(
        W,
        FrameId(seq),
        &[
            (Stage::Draw, t0 + Span::from_millis(7)),
            (Stage::Submit, t0 + Span::from_millis(8)),
            (Stage::GpuDone, t0 + Span::from_millis(10)),
            (Stage::Present, t0 + Span::from_millis(11)),
        ],
    );
    r.presented(
        W,
        FrameId(seq),
        t0 + Span::from_millis(16),
        false,
        &[InputSeq(seq)],
    );
}

#[test]
fn a_full_pipeline_terminates_presented_with_every_segment() {
    let mut r = LatencyRecorder::default();
    full_pipeline(&mut r, 1, ms(100));
    assert_eq!(r.live_count(), 0);
    assert_eq!(
        r.termination(InputSeq(1)),
        Some(Termination::Presented(FrameId(1)))
    );
    let rec = r.record(InputSeq(1)).unwrap();
    for s in skelter::latency::Stage::ALL {
        assert!(rec.at(s).is_some(), "stage {s:?} stamped");
    }
    for seg in Segment::ALL {
        let h = r
            .histogram((seg, InputClass::Key, W))
            .unwrap_or_else(|| panic!("{seg:?} recorded"));
        assert_eq!(h.count(), 1);
    }
    let total = r
        .histogram((Segment::InputToPresented, InputClass::Key, W))
        .unwrap();
    assert_eq!(total.max(), Some(Span::from_millis(16)));
    assert!(r.violations().is_empty());
    assert_eq!(r.counters(W).presented, 1);
}

#[test]
fn presented_folds_every_older_committed_input() {
    let mut r = LatencyRecorder::default();
    for s in 1..=3 {
        r.begin(&key(s, ms(s)));
        r.stamp(InputSeq(s), Stage::DispatchEnd, ms(s));
        r.stamp_pending(W, Stage::DispatchEnd, Stage::UiFrameStart, ms(s));
        r.commit_sent(W, Revision(s), ms(s));
    }
    // The compositor folded the three commits and names only the newest input.
    r.presented(W, FrameId(9), ms(20), true, &[InputSeq(3)]);
    for s in 1..=3 {
        assert_eq!(
            r.termination(InputSeq(s)),
            Some(Termination::Presented(FrameId(9)))
        );
        assert!(r.record(InputSeq(s)).unwrap().estimated);
    }
    assert_eq!(r.counters(W).estimated, 1);
}

#[test]
fn a_second_termination_is_a_violation_not_a_second_end() {
    let mut r = LatencyRecorder::default();
    r.begin(&key(1, ms(1)));
    r.no_visual_change(InputSeq(1));
    r.merged(InputSeq(1), InputSeq(2));
    assert_eq!(
        r.termination(InputSeq(1)),
        Some(Termination::NoVisualChange)
    );
    assert_eq!(r.terminated_count(), 1);
    assert!(matches!(
        r.violations(),
        [Violation::TerminatedTwice {
            seq: InputSeq(1),
            ..
        }]
    ));
}

#[test]
fn stamps_after_the_end_and_unknown_seqs_are_violations() {
    let mut r = LatencyRecorder::default();
    r.begin(&key(1, ms(1)));
    r.no_visual_change(InputSeq(1));
    r.stamp(InputSeq(1), Stage::Commit, ms(2));
    r.stamp(InputSeq(7), Stage::Commit, ms(2));
    r.begin(&key(1, ms(3)));
    assert_eq!(
        r.violations(),
        &[
            Violation::StampAfterEnd {
                seq: InputSeq(1),
                stage: Stage::Commit
            },
            Violation::Unknown { seq: InputSeq(7) },
            Violation::DuplicateBegin { seq: InputSeq(1) },
        ]
    );
}

#[test]
fn stale_records_drop_as_timeouts() {
    let mut r = LatencyRecorder::new(LatencyConfig {
        stale_after: Span::from_millis(100),
        ..LatencyConfig::default()
    });
    r.begin(&key(1, ms(0)));
    r.begin(&key(2, ms(150)));
    r.sweep(ms(160));
    assert_eq!(
        r.termination(InputSeq(1)),
        Some(Termination::Dropped(DropReason::Timeout))
    );
    assert_eq!(r.termination(InputSeq(2)), None);
    r.drop_window(W);
    assert_eq!(
        r.termination(InputSeq(2)),
        Some(Termination::Dropped(DropReason::WindowGone))
    );
    assert_eq!(r.live_count(), 0);
}

#[test]
fn a_frame_without_a_commit_ends_its_inputs_as_no_visual_change() {
    let mut r = LatencyRecorder::default();
    r.begin(&key(1, ms(1)));
    r.stamp(InputSeq(1), Stage::DispatchEnd, ms(1));
    r.stamp_pending(W, Stage::DispatchEnd, Stage::UiFrameStart, ms(2));
    r.frame_without_commit(W);
    assert_eq!(
        r.termination(InputSeq(1)),
        Some(Termination::NoVisualChange)
    );
}

#[test]
fn fast_commits_carry_inputs_without_a_ui_frame() {
    let mut r = LatencyRecorder::default();
    r.begin(&wheel(1, 10.0, None));
    r.stamp(InputSeq(1), Stage::DispatchEnd, ms(1));
    r.fast_sent(InputSeq(1), Revision(4), ms(1));
    r.presented(W, FrameId(2), ms(10), false, &[InputSeq(1)]);
    assert_eq!(
        r.termination(InputSeq(1)),
        Some(Termination::Presented(FrameId(2)))
    );
    assert!(
        r.record(InputSeq(1))
            .unwrap()
            .at(Stage::UiFrameStart)
            .is_none()
    );
}

#[test]
fn the_ended_set_stays_bounded() {
    let mut r = LatencyRecorder::new(LatencyConfig {
        keep_records: 8,
        ..LatencyConfig::default()
    });
    for s in 1..=100 {
        r.begin(&key(s, ms(s)));
        r.no_visual_change(InputSeq(s));
    }
    assert_eq!(r.finished().count(), 8);
    assert_eq!(r.terminated_count(), 100);
    assert!(r.violations().is_empty());
}

#[test]
fn histogram_quantiles_have_two_significant_digits() {
    let mut h = Histogram::new();
    for us in 1..=10_000i64 {
        h.record(Span::from_micros(us));
    }
    for (q, exact) in [(0.5, 5_000.0), (0.95, 9_500.0), (0.99, 9_900.0)] {
        let got = h.quantile(q).unwrap().0 as f64 / 1_000.0;
        assert!((got - exact).abs() / exact < 0.01, "q{q}: {got} vs {exact}");
    }
    assert_eq!(h.min(), Some(Span::from_micros(1)));
    assert_eq!(h.max(), Some(Span::from_micros(10_000)));
    let mut big = Histogram::new();
    big.record(Span::from_millis(60_000));
    assert!(
        big.max().unwrap() <= Span::from_millis(16_778),
        "clamped into range"
    );
    let mut m = Histogram::new();
    m.merge(&h);
    m.merge(&big);
    assert_eq!(m.count(), 10_001);
}

#[test]
fn snapshot_and_trace_are_json_shaped() {
    let mut r = LatencyRecorder::default();
    full_pipeline(&mut r, 1, ms(100));
    r.slice(skelter::latency::Track::Ui, "ui_frame", W, ms(103), ms(105));
    let snap = r.snapshot_json();
    assert!(snap.starts_with("{\"histograms\":["));
    assert!(snap.contains("\"segment\":\"InputToPresented\""));
    assert!(snap.ends_with("\"violations\":0}"));
    let trace = r.chrome_trace();
    let flows = ["\"ph\":\"s\"", "\"ph\":\"t\"", "\"ph\":\"f\""];
    for f in flows {
        assert!(trace.contains(f), "{f}");
    }
    assert_eq!(trace.matches("\"ph\":\"s\"").count(), 1);
    assert_eq!(trace.matches("\"ph\":\"f\"").count(), 1);
    // Async begin/end pairs balance.
    assert_eq!(
        trace.matches("\"ph\":\"b\"").count(),
        trace.matches("\"ph\":\"e\"").count()
    );
    assert!(trace.contains("\"ts\":100000.000"));
}

#[test]
fn pointer_moves_merge_with_history() {
    let mut c = Coalescer::new();
    assert_eq!(
        c.push(pointer(1, ms(1), PointerPhase::Move, 1.0, 0)),
        Pushed::Held
    );
    assert_eq!(
        c.push(pointer(2, ms(2), PointerPhase::Move, 2.0, 0)),
        Pushed::Merged { old: InputSeq(1) }
    );
    assert_eq!(
        c.push(pointer(3, ms(3), PointerPhase::Move, 3.0, 0)),
        Pushed::Merged { old: InputSeq(2) }
    );
    // Different buttons: a different stream.
    assert_eq!(
        c.push(pointer(4, ms(4), PointerPhase::Move, 4.0, 1)),
        Pushed::Held
    );
    let out = c.take(W);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].seq, InputSeq(3));
    let InputEvent::Pointer(p) = &out[0].event else {
        panic!()
    };
    assert_eq!(p.pos.x, 3.0);
    let xs: Vec<_> = p.history.iter().map(|s| (s.pos.x, s.time)).collect();
    assert_eq!(xs, vec![(1.0, ms(1)), (2.0, ms(2))]);
    assert!(!c.has_pending(W));
}

#[test]
fn wheel_deltas_add_but_gesture_boundaries_stay() {
    let mut c = Coalescer::new();
    c.push(wheel(1, 10.0, Some(GesturePhase::Started)));
    assert_eq!(
        c.push(wheel(2, 5.0, Some(GesturePhase::Moved))),
        Pushed::Held
    );
    assert_eq!(
        c.push(wheel(3, 7.0, Some(GesturePhase::Moved))),
        Pushed::Merged { old: InputSeq(2) }
    );
    assert_eq!(
        c.push(wheel(4, 0.0, Some(GesturePhase::Ended))),
        Pushed::Held
    );
    let out = c.take(W);
    assert_eq!(out.len(), 3);
    let InputEvent::Wheel(w) = &out[1].event else {
        panic!()
    };
    assert_eq!(w.delta, ScrollDelta::Pixels(Vec2::new(0.0, 12.0)));
}

#[test]
fn raw_motion_deltas_sum_and_stay_continuous() {
    let raw = |seq: u64, dx: f32, dy: f32| InputEnvelope {
        seq: InputSeq(seq),
        window: W,
        os_time: ms(seq),
        recv_time: ms(seq),
        event: InputEvent::RawMotion { dx, dy },
    };
    assert!(!raw(1, 0.0, 0.0).event.is_discrete());
    let mut c = Coalescer::new();
    assert_eq!(c.push(raw(1, 3.0, -1.0)), Pushed::Held);
    assert_eq!(c.push(raw(2, 2.0, 4.0)), Pushed::Merged { old: InputSeq(1) });
    assert_eq!(c.push(raw(3, -1.0, 0.5)), Pushed::Merged { old: InputSeq(2) });
    // A pointer move in between starts a new stream.
    c.push(pointer(4, ms(4), PointerPhase::Move, 1.0, 0));
    assert_eq!(c.push(raw(5, 1.0, 1.0)), Pushed::Held);
    let out = c.take(W);
    assert_eq!(out.len(), 3);
    assert_eq!(out[0].seq, InputSeq(3));
    assert_eq!(out[0].event, InputEvent::RawMotion { dx: 4.0, dy: 3.5 });
    assert_eq!(out[2].event, InputEvent::RawMotion { dx: 1.0, dy: 1.0 });
}

#[test]
fn discrete_events_are_never_coalesced() {
    // The app sends discrete events straight to dispatch; the coalescer still refuses to merge them.
    let mut c = Coalescer::new();
    c.push(pointer(1, ms(1), PointerPhase::Down, 1.0, 1));
    assert_eq!(
        c.push(pointer(2, ms(2), PointerPhase::Down, 1.0, 1)),
        Pushed::Held
    );
    assert_eq!(c.take(W).len(), 2);
}

#[test]
fn frame_timings_after_presented_amend_the_record() {
    let mut r = LatencyRecorder::default();
    let t0 = ms(100);
    r.begin(&key(1, t0));
    r.stamp(InputSeq(1), Stage::DispatchEnd, t0 + Span::from_millis(1));
    r.stamp_pending(
        W,
        Stage::DispatchEnd,
        Stage::UiFrameStart,
        t0 + Span::from_millis(2),
    );
    r.commit_sent(W, Revision(1), t0 + Span::from_millis(3));
    r.commit_applied(W, Revision(1), t0 + Span::from_millis(4));
    r.presented(
        W,
        FrameId(5),
        t0 + Span::from_millis(16),
        false,
        &[InputSeq(1)],
    );
    assert!(
        r.histogram((Segment::DrawToSubmit, InputClass::Key, W))
            .is_none()
    );
    // The compositor's `FrameTimings` follows `Presented`.
    r.frame_stages(
        W,
        FrameId(5),
        &[
            (Stage::Draw, t0 + Span::from_millis(5)),
            (Stage::Submit, t0 + Span::from_millis(7)),
            (Stage::Present, t0 + Span::from_millis(9)),
        ],
    );
    let rec = r.record(InputSeq(1)).unwrap();
    assert_eq!(rec.at(Stage::Submit), Some(t0 + Span::from_millis(7)));
    let h = r
        .histogram((Segment::DrawToSubmit, InputClass::Key, W))
        .unwrap();
    assert_eq!((h.count(), h.max()), (1, Some(Span::from_millis(2))));
    assert_eq!(
        r.histogram((Segment::InputToPresented, InputClass::Key, W))
            .unwrap()
            .count(),
        1
    );
    assert!(r.violations().is_empty());
}
