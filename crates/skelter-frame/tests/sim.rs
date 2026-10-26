//! Scheduler scenarios under the deterministic simulator (`round2/scheduler-latency.md` §6.2):
//! invariants over seeded runs, latency in frames, and the fault table's expectations.

use skelter_base::time::{Span, Time};
use skelter_frame::sim::{self, Cost, DisplayMode, Fault, SimConfig, WindowCfg};

const START: Time = Time(1_000_000_000);

fn ms(n: i64) -> Time {
    START + Span::from_millis(n)
}

fn busy(seed: u64) -> SimConfig {
    let mut c = SimConfig::single(seed);
    let w = &mut c.windows[0];
    w.input.discrete_hz = 6.0;
    w.comp_anim = vec![(ms(200), ms(900)), (ms(1_800), ms(2_400))];
    w.ui_anim = vec![(ms(500), ms(1_300))];
    w.input.streams = vec![(ms(1_000), ms(1_600), 120.0)];
    c
}

#[test]
fn seeded_runs_hold_every_invariant() {
    for seed in sim::seeds(24) {
        let r = sim::run(&busy(seed));
        r.assert_ok();
        assert!(r.windows[0].shown > 50, "seed {seed}: only {} frames shown", r.windows[0].shown);
    }
}

#[test]
fn lost_feedback_without_buffer_release_times_out() {
    // A platform that reports neither feedback nor buffer availability still makes progress.
    let mut c = SimConfig::single(4);
    c.windows[0].comp_anim = vec![(ms(0), ms(1_000))];
    c.faults = vec![Fault::DropFeedback { p: 1.0 }, Fault::NoBufferRelease];
    let r = sim::run(&c);
    r.assert_ok();
    let w = &r.windows[0];
    assert!(w.stats.feedback_timeouts > 0);
    // one draw per feedback timeout (3 intervals) at worst: about 15 in a second at 60 Hz
    assert!(w.stats.draws >= 12, "{} draws", w.stats.draws);
}

#[test]
fn discrete_input_presents_within_one_or_two_vblanks() {
    let mut all = Vec::new();
    for seed in sim::seeds(12) {
        let mut c = SimConfig::single(seed);
        c.windows[0].input.discrete_hz = 5.0;
        c.duration = Span::from_millis(6_000);
        let r = sim::run(&c);
        r.assert_ok();
        assert!(r.inputs.len() > 10);
        assert!(r.inputs.iter().all(|i| i.presented.is_some() || i.at > ms(5_750)), "seed {seed}");
        let p90 = r.frames_percentile(0.90).unwrap();
        assert!(p90 <= 1, "seed {seed}: p90 latency {p90} frames");
        all.extend(r.inputs.iter().filter_map(|i| i.frames));
    }
    // §5.6: ≤ +1 vblank typically; ≤ +2 for an input that arrives while the UI is still busy
    // with the previous one (its commit already drawn for the next vblank); a cost overrun on
    // top of that adds one more, rarely.
    all.sort_unstable();
    let p99 = all[(all.len() * 99).div_ceil(100) - 1];
    assert!(p99 <= 2, "p99 latency {p99} frames");
    assert!(*all.last().unwrap() <= 3, "max latency {} frames", all.last().unwrap());
}

#[test]
fn idle_window_has_no_wakeups() {
    let r = sim::run(&SimConfig::single(7));
    r.assert_ok();
    let w = &r.windows[0];
    assert_eq!(w.timer_wakeups, 0);
    assert_eq!(w.ticks_delivered, 0);
    assert_eq!(w.stats.draws, 0);
}

#[test]
fn idle_after_activity_unsubscribes() {
    let mut c = SimConfig::single(3);
    c.windows[0].input.script = vec![ms(100)];
    c.windows[0].comp_anim = vec![(ms(300), ms(400))];
    let r = sim::run(&c);
    r.assert_ok();
    let w = &r.windows[0];
    // ~6 animation frames plus the key's frame plus a quiet tick each time; nothing over 2.6 s idle
    assert!(w.ticks_delivered < 20, "{} ticks", w.ticks_delivered);
    assert_eq!(w.idle_wakeups, 0);
}

#[test]
fn slow_js_keeps_compositor_frames_on_time() {
    for seed in sim::seeds(8) {
        let mut c = SimConfig::single(seed);
        c.windows[0].comp_anim = vec![(ms(0), ms(3_000))];
        c.windows[0].ui_anim = vec![(ms(0), ms(3_000))];
        c.faults = vec![Fault::SlowUiFrame { window: 0, at: ms(1_000), cost: Span::from_millis(40) }];
        let r = sim::run(&c);
        r.assert_ok();
        let s = &r.windows[0].stats;
        assert!(s.ui_missed >= 1, "seed {seed}: the slow frame misses its deadline");
        // a 60 Hz compositor animation for 3 s: no more than a handful of frames lost to the slow
        // UI frame (two-deadline rule)
        assert!(r.windows[0].shown >= 170, "seed {seed}: {} shown", r.windows[0].shown);
    }
}

#[test]
fn sustained_slow_ui_pipelines_and_recovers() {
    for seed in sim::seeds(6) {
        let mut c = SimConfig::single(seed);
        c.windows[0].ui_anim = vec![(ms(0), ms(3_000))];
        c.windows[0].comp_anim = vec![(ms(0), ms(3_000))];
        c.faults = vec![Fault::SlowUi { from: ms(500), to: ms(1_500), cost: Cost::lognormal(Span::from_millis(13), 0.1) }];
        let r = sim::run(&c);
        r.assert_ok();
        let s = &r.windows[0].stats;
        assert!(s.commits_applied > 100, "seed {seed}: {} commits", s.commits_applied);
        assert!(r.windows[0].shown >= 165, "seed {seed}: compositor stays at full rate ({} shown)", r.windows[0].shown);
    }
}

#[test]
fn gpu_spike_misses_once_and_recovers() {
    let mut c = SimConfig::single(11);
    c.windows[0].comp_anim = vec![(ms(0), ms(3_000))];
    c.faults = vec![Fault::GpuSpike { at: ms(1_000), cost: Span::from_millis(25) }];
    let r = sim::run(&c);
    r.assert_ok();
    let s = &r.windows[0].stats;
    assert!(s.comp_missed >= 1);
    assert!(s.comp_missed <= 3, "{} misses", s.comp_missed);
}

#[test]
fn dropped_and_discarded_feedback_never_wedge() {
    for seed in sim::seeds(8) {
        let mut c = busy(seed);
        c.windows[0].comp_anim = vec![(ms(0), ms(3_000))];
        c.faults = vec![Fault::DropFeedback { p: 0.1 }, Fault::Discard { p: 0.05 }];
        let r = sim::run(&c);
        r.assert_ok();
        // buffer availability releases the frame when its feedback is lost
        let w = &r.windows[0];
        assert!(w.stats.draws > 160, "seed {seed}: {} draws", w.stats.draws);
    }
}

#[test]
fn late_timers_never_double_draw() {
    for seed in sim::seeds(8) {
        let mut c = busy(seed);
        c.timer.late_p = 0.1;
        c.timer.late = Span::from_millis(2);
        sim::run(&c).assert_ok();
    }
}

#[test]
fn output_change_mid_drag() {
    for seed in sim::seeds(6) {
        let mut c = busy(seed);
        c.windows[0].comp_anim = vec![(ms(0), ms(3_000))];
        c.windows[0].input.streams = vec![(ms(800), ms(2_000), 250.0)];
        c.faults = vec![Fault::OutputChange { window: 0, at: ms(1_200), interval: Span::from_secs_f64(1.0 / 144.0) }];
        let r = sim::run(&c);
        r.assert_ok();
        // 1.2 s at 60 Hz + 1.8 s at 144 Hz
        assert!(r.windows[0].shown > 250, "seed {seed}: {} shown", r.windows[0].shown);
    }
}

#[test]
fn hidden_window_stops_and_shows_once() {
    for seed in sim::seeds(6) {
        let mut c = busy(seed);
        c.faults = vec![Fault::Hide { window: 0, at: ms(700), until: ms(2_000) }];
        let r = sim::run(&c);
        r.assert_ok();
        assert!(r.inputs.iter().filter(|i| i.at > ms(710) && i.at < ms(1_990)).all(|i| i.dropped || i.presented.is_some_and(|p| p >= ms(2_000))));
    }
}

#[test]
fn two_windows_on_different_displays_share_the_gpu() {
    for seed in sim::seeds(6) {
        let mut c = busy(seed);
        let mut w2 = WindowCfg::hz(144.0);
        w2.comp_anim = vec![(ms(0), ms(3_000))];
        w2.input.discrete_hz = 4.0;
        c.windows.push(w2);
        let r = sim::run(&c);
        r.assert_ok();
        assert!(r.windows[1].shown > 380, "seed {seed}: 144 Hz window showed {}", r.windows[1].shown);
    }
}

#[test]
fn device_loss_forces_a_full_redraw() {
    let mut c = SimConfig::single(5);
    c.windows[0].comp_anim = vec![(ms(0), ms(3_000))];
    c.faults = vec![Fault::DeviceLost { window: 0, at: ms(1_000), failed_draws: 3 }];
    let r = sim::run(&c);
    r.assert_ok();
    assert_eq!(r.windows[0].failed_draws_injected, 3);
    assert!(r.windows[0].stats.forced_redraws >= 1);
}

#[test]
fn vrr_keeps_presenting_after_high_rate_input() {
    let mut c = SimConfig::single(9);
    let w = &mut c.windows[0];
    w.display.mode = DisplayMode::Variable { min_interval: Span::from_secs_f64(1.0 / 120.0) };
    w.display.interval = Span::from_secs_f64(1.0 / 120.0);
    w.clock.interval = w.display.interval;
    w.clock.vrr_min_interval = Some(w.display.interval);
    w.input.streams = vec![(ms(500), ms(800), 500.0)];
    let r = sim::run(&c);
    r.assert_ok();
    assert!(r.windows[0].stats.represents > 60, "{} re-presents", r.windows[0].stats.represents);
}

#[test]
fn headless_blocked_mode_draws_every_commit() {
    let mut c = busy(2);
    c.windows[0].clock.blocked = true;
    let r = sim::run(&c);
    r.assert_ok();
    assert!(r.inputs.iter().all(|i| i.presented.is_some() || i.at > ms(2_750)));
}

#[test]
fn the_same_seed_gives_the_same_run() {
    let a = sim::run(&busy(42));
    let b = sim::run(&busy(42));
    assert_eq!(a, b);
}
