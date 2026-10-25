//! Render-time estimation (`round2/scheduler-latency.md` §3.2): the p90 of a 64-sample ring per cost
//! series with a 0.25 ms floor and outliers capped at two intervals, the per-window latch margin
//! (KWin's DRM rule: fast attack, slow decay), and the start/deadline arithmetic built on them.

use presto_base::time::{Span, Time};

/// Samples per series: about 0.5 s at 120 Hz (Chromium keeps 60).
pub const RING: usize = 64;
/// The percentile every estimate reads (Chromium `kDrawEstimationPercentile`).
pub const PERCENTILE: f64 = 0.90;
/// The lowest estimate of any series. KWin's 2 ms floor overestimates fast renderers 5× (farnoy).
pub const FLOOR: Span = Span::from_micros(250);
/// The commit hand-off between the UI and the compositor thread.
pub const EPS_COMMIT: Span = Span::from_micros(50);
/// The margin cap: `min(MARGIN_CAP, interval / 2)`.
pub const MARGIN_CAP: Span = Span::from_millis(3);
/// A frame that costs more than this multiple of its estimate makes the next frames provisional.
pub const PROVISIONAL_FACTOR: i64 = 2;
/// How many frames stay provisional (`max(q, last)`) after such a spike.
pub const PROVISIONAL_FRAMES: u32 = 8;

/// Cold-start seeds (architecture §10.2).
pub const SEED_UI: Span = Span::from_millis(2);
pub const SEED_COMP_CPU: Span = Span::from_millis(1);
pub const SEED_GPU: Span = Span::from_millis(2);
pub const SEED_MARGIN: Span = Span::from_millis(1);

/// One cost series: a ring of the last [`RING`] samples.
#[derive(Clone, Debug)]
pub struct Series {
    ring: [i64; RING],
    len: usize,
    next: usize,
    seed: Span,
    last: Span,
    tracks_spikes: bool,
    provisional: u32,
}

impl Series {
    pub fn new(seed: Span) -> Self {
        Series { ring: [0; RING], len: 0, next: 0, seed, last: Span::ZERO, tracks_spikes: false, provisional: 0 }
    }

    /// A series whose spikes make the next [`PROVISIONAL_FRAMES`] estimates `max(q, last)` (the UI
    /// series: a heavy panel that opens costs the same next frame).
    pub fn with_spike_tracking(seed: Span) -> Self {
        Series { tracks_spikes: true, ..Series::new(seed) }
    }

    /// Adds a sample, capped at `2 × interval` so one hitch cannot pin the estimate.
    pub fn record(&mut self, sample: Span, interval: Span) {
        let cap = Span(interval.0.saturating_mul(2).max(FLOOR.0));
        let s = Span(sample.0.clamp(0, cap.0));
        if self.tracks_spikes {
            let est = self.estimate();
            if est.0 > 0 && s.0 > est.0.saturating_mul(PROVISIONAL_FACTOR) {
                self.provisional = PROVISIONAL_FRAMES;
            } else if self.provisional > 0 {
                self.provisional -= 1;
            }
        }
        self.ring[self.next] = s.0;
        self.next = (self.next + 1) % RING;
        self.len = (self.len + 1).min(RING);
        self.last = s;
    }

    pub fn samples(&self) -> usize {
        self.len
    }

    /// `max(FLOOR, p90)`, or the seed before the first sample; `max(q, last)` while provisional.
    pub fn estimate(&self) -> Span {
        if self.len == 0 {
            return self.seed;
        }
        let q = self.percentile(PERCENTILE);
        let q = if self.provisional > 0 { q.max(self.last) } else { q };
        q.max(FLOOR)
    }

    /// Nearest-rank percentile of the samples held.
    pub fn percentile(&self, p: f64) -> Span {
        if self.len == 0 {
            return self.seed;
        }
        let mut v: Vec<i64> = self.ring[..self.len].to_vec();
        v.sort_unstable();
        let rank = ((p * self.len as f64).ceil() as usize).clamp(1, self.len);
        Span(v[rank - 1])
    }

    pub fn reset(&mut self) {
        *self = Series { tracks_spikes: self.tracks_spikes, ..Series::new(self.seed) };
    }
}

/// The latch margin `m` of one window.
#[derive(Clone, Copy, Debug)]
pub struct Margin {
    pub value: Span,
}

impl Margin {
    pub fn new() -> Self {
        Margin { value: SEED_MARGIN }
    }

    pub fn cap(interval: Span) -> Span {
        MARGIN_CAP.min(Span(interval.0 / 2))
    }

    /// `slack = (T − L_latch_used) − t_present_call`. A miss raises `m` by the overshoot at once (at
    /// least [`FLOOR`], since a miss with positive slack means the latch is earlier than assumed); a
    /// hit lowers it by a tenth of the slack it had.
    pub fn update(&mut self, slack: Span, missed: bool, interval: Span) {
        let cap = Self::cap(interval);
        let v = if missed {
            self.value + Span((-slack.0).max(FLOOR.0))
        } else {
            let give = slack.0.max(0).min(self.value.0);
            Span(self.value.0 - give / 10)
        };
        self.value = Span(v.0.clamp(0, cap.0));
    }
}

impl Default for Margin {
    fn default() -> Self {
        Margin::new()
    }
}

/// The cost series of one window plus the UI-thread series (shared by all windows in a real app; the
/// compositor keeps one copy per clock and the UI reports into every clock it commits to).
#[derive(Clone, Debug)]
pub struct Estimator {
    /// UI frames that contain discrete input (the urgent check reads this one).
    pub ui_input: Series,
    /// Other UI frames (animation, rAF, async results).
    pub ui_other: Series,
    pub comp_cpu: Series,
    pub gpu: Series,
    pub margin: Margin,
    /// How late the scheduler's timer wakes (`wake_error = actual − armed`); its p95 floors the
    /// margin, so a coarse timer means an earlier start rather than missed frames (§4).
    pub wake_error: Series,
}

/// One target's schedule: when the UI starts, when it must commit, when the compositor draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Schedule {
    pub target: Time,
    pub s_ui: Time,
    pub d_ui: Time,
    pub d_comp: Time,
}

impl Estimator {
    pub fn new() -> Self {
        Estimator {
            ui_input: Series::with_spike_tracking(SEED_UI),
            ui_other: Series::with_spike_tracking(SEED_UI),
            comp_cpu: Series::new(SEED_COMP_CPU),
            gpu: Series::new(SEED_GPU),
            margin: Margin::new(),
            wake_error: Series::new(Span::ZERO),
        }
    }

    /// The margin in use: `max(m, p95(wake_error))`.
    pub fn margin(&self) -> Span {
        let wake = if self.wake_error.samples() == 0 { Span::ZERO } else { self.wake_error.percentile(0.95) };
        self.margin.value.max(wake)
    }

    pub fn est_ui(&self) -> Span {
        self.ui_other.estimate()
    }

    pub fn est_ui_input(&self) -> Span {
        self.ui_input.estimate()
    }

    /// Compositor CPU plus GPU.
    pub fn est_comp(&self) -> Span {
        self.comp_cpu.estimate() + self.gpu.estimate()
    }

    /// Everything from the UI start to the latch, clamped to two intervals (KWin, Mutter).
    pub fn est_total(&self, latch: Span, interval: Span) -> Span {
        let t = latch + self.margin() + self.est_comp() + EPS_COMMIT + self.est_ui();
        t.min(Span(interval.0.saturating_mul(2)))
    }

    /// `D_comp = T − (L_latch + m) − est_gpu − est_comp_cpu`, `D_ui = D_comp − ε_commit`,
    /// `S_ui = D_ui − est_ui`.
    pub fn schedule(&self, target: Time, latch: Span) -> Schedule {
        self.schedule_with(target, latch, self.est_ui())
    }

    /// [`Estimator::schedule`] for a UI frame of cost `est_ui` (frames with discrete input use
    /// `est_ui_input`).
    pub fn schedule_with(&self, target: Time, latch: Span, est_ui: Span) -> Schedule {
        let d_comp = target - (latch + self.margin()) - self.gpu.estimate() - self.comp_cpu.estimate();
        let d_ui = d_comp - EPS_COMMIT;
        let s_ui = d_ui - est_ui;
        Schedule { target, s_ui, d_ui, d_comp }
    }

    /// Forgets every sample (GPU device loss, backend change).
    pub fn reset(&mut self) {
        *self = Estimator::new();
    }
}

impl Default for Estimator {
    fn default() -> Self {
        Estimator::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const I: Span = Span::from_micros(8_333);

    #[test]
    fn seed_until_first_sample() {
        let s = Series::new(SEED_UI);
        assert_eq!(s.estimate(), SEED_UI);
    }

    #[test]
    fn p90_with_floor() {
        let mut s = Series::new(SEED_UI);
        for i in 1..=10 {
            s.record(Span::from_micros(100 * i), I);
        }
        // p90 of 0.1..1.0 ms is 0.9 ms.
        assert_eq!(s.estimate(), Span::from_micros(900));
        let mut f = Series::new(SEED_UI);
        for _ in 0..RING {
            f.record(Span::from_micros(10), I);
        }
        assert_eq!(f.estimate(), FLOOR);
    }

    #[test]
    fn outlier_capped_and_flushed_in_one_rotation() {
        let mut s = Series::new(SEED_GPU);
        for _ in 0..RING {
            s.record(Span::from_millis(1), I);
        }
        s.record(Span::from_millis(40), I);
        assert!(s.estimate() <= Span(I.0 * 2));
        for _ in 0..RING {
            s.record(Span::from_millis(1), I);
        }
        assert_eq!(s.estimate(), Span::from_millis(1));
    }

    #[test]
    fn margin_attack_and_decay() {
        let mut m = Margin::new();
        m.update(Span::from_micros(-1500), true, I);
        assert_eq!(m.value, Span::from_micros(2500));
        // decays by about 90% within 22 hits of large slack
        for _ in 0..22 {
            m.update(Span::from_millis(5), false, I);
        }
        assert!(m.value.0 < Span::from_micros(2500).0 / 9);
        // clamp
        m.update(Span::from_millis(-20), true, I);
        assert_eq!(m.value, Margin::cap(I));
    }
}
