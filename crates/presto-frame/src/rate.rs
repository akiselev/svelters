//! High-rate input detection for VRR and ProMotion keep-alive (architecture §10.3), after GPUI's
//! `InputRateTracker` (`crates/gpui/src/window.rs`, zed-industries/zed@a5c4ca8b): input is high-rate
//! when at least `inputs_per_second × window` events arrived within `window`, and stays so for
//! `sustain` after the last qualifying event.

use std::collections::VecDeque;

use presto_base::time::{Span, Time};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RateConfig {
    pub window: Span,
    pub inputs_per_second: u32,
    pub sustain: Span,
}

impl Default for RateConfig {
    /// GPUI's defaults: 100 ms window, 60 events/s, sustained 1 s.
    fn default() -> Self {
        RateConfig { window: Span::from_millis(100), inputs_per_second: 60, sustain: Span::from_millis(1000) }
    }
}

#[derive(Clone, Debug)]
pub struct InputRateTracker {
    cfg: RateConfig,
    times: VecDeque<Time>,
    sustain_until: Option<Time>,
}

impl InputRateTracker {
    pub fn new(cfg: RateConfig) -> Self {
        InputRateTracker { cfg, times: VecDeque::new(), sustain_until: None }
    }

    /// Events needed inside one window (6 for the defaults).
    pub fn threshold(&self) -> usize {
        ((self.cfg.inputs_per_second as f64 * self.cfg.window.as_secs_f64()).round() as usize).max(1)
    }

    pub fn record(&mut self, at: Time) {
        let start = at - self.cfg.window;
        while self.times.front().is_some_and(|&t| t < start) {
            self.times.pop_front();
        }
        self.times.push_back(at);
        if self.times.len() >= self.threshold() {
            self.sustain_until = Some(at + self.cfg.sustain);
        }
    }

    pub fn is_high_rate(&self, now: Time) -> bool {
        self.sustain_until.is_some_and(|u| now < u)
    }

    /// When the sustain lapses, if it is running.
    pub fn sustain_until(&self) -> Option<Time> {
        self.sustain_until
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_in_100ms_sustains_one_second() {
        let mut r = InputRateTracker::new(RateConfig::default());
        assert_eq!(r.threshold(), 6);
        let t0 = Time(1_000_000_000);
        for i in 0..5 {
            r.record(t0 + Span::from_millis(16 * i));
        }
        assert!(!r.is_high_rate(t0 + Span::from_millis(80)));
        r.record(t0 + Span::from_millis(90));
        let last = t0 + Span::from_millis(90);
        assert!(r.is_high_rate(last + Span::from_millis(999)));
        assert!(!r.is_high_rate(last + Span::from_millis(1000)));
    }

    #[test]
    fn slow_input_never_qualifies() {
        let mut r = InputRateTracker::new(RateConfig::default());
        for i in 0..100 {
            r.record(Time(0) + Span::from_millis(30 * i));
        }
        assert!(!r.is_high_rate(Time(0) + Span::from_millis(3000)));
    }
}
