//! E2: `svelters:app/dsp`, large numeric data without copies.
//!
//! - `noise(n, seed)` makes samples in Rust and hands the memory to JS (`Float32Array`).
//! - `smooth(samples, passes)` moves the array to a worker (JS's array is detached meanwhile), filters it
//!   in place and hands the same memory back.
//! - `waveform()` is a read-only view of an `Arc<[f32]>` Rust keeps (`ReadonlyFloat32Array`).
//! - `stats(view)` reads a view without a copy when it is one Rust handed out.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use svelters::{Buffer, Shared};

/// Summary statistics of a signal.
#[derive(svelters::Record, Clone, Debug)]
pub struct SignalStats {
    pub len: u32,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
}

/// Where the last `noise` allocation lives (to show that `smooth` gets that very memory back).
static LAST_NOISE: AtomicUsize = AtomicUsize::new(0);

/// `n` samples of uniform noise in -1..1 (a small xorshift generator; deterministic per `seed`).
#[svelters::export(module = "dsp")]
pub fn noise(n: u32, seed: u32) -> Buffer<f32> {
    let mut x = seed.max(1);
    let mut out = Buffer::zeroed(n as usize);
    LAST_NOISE.store(out.as_ptr() as usize, Ordering::Relaxed);
    for s in out.iter_mut() {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        *s = (x as f32 / u32::MAX as f32) * 2.0 - 1.0;
    }
    out
}

/// What `smooth` returns: the filtered samples (the input's memory) and the time it took.
#[derive(svelters::Record)]
pub struct Smoothed {
    pub samples: Buffer<f32>,
    pub ms: f64,
    /// True when the samples are the allocation `noise` made (JS -> worker -> JS without a copy).
    pub zero_copy: bool,
}

/// Smooths `samples` in place with `passes` 3-tap moving averages on a worker thread. The caller's
/// array is detached (moved to Rust) until the result comes back as the same memory.
#[svelters::export(module = "dsp")]
#[js(worker)]
pub fn smooth(mut samples: Buffer<f32>, passes: u32, cx: svelters::WorkCx<f64>) -> Result<Smoothed, svelters::Cancelled> {
    let t0 = std::time::Instant::now();
    let zero_copy = samples.as_ptr() as usize == LAST_NOISE.load(Ordering::Relaxed);
    for p in 0..passes {
        cx.check()?;
        let s = &mut samples[..];
        let mut prev = s.first().copied().unwrap_or(0.0);
        for i in 1..s.len().saturating_sub(1) {
            let cur = s[i];
            s[i] = (prev + cur + s[i + 1]) / 3.0;
            prev = cur;
        }
        cx.progress((p + 1) as f64 / passes.max(1) as f64);
    }
    Ok(Smoothed { samples, ms: t0.elapsed().as_secs_f64() * 1e3, zero_copy })
}

fn wave() -> &'static Arc<[f32]> {
    static W: OnceLock<Arc<[f32]>> = OnceLock::new();
    W.get_or_init(|| (0..4096).map(|i| (i as f32 * std::f32::consts::TAU / 512.0).sin()).collect::<Vec<_>>().into())
}

/// A sine table Rust keeps, as a read-only view (no copy; the view holds the `Arc` until JS collects it).
#[svelters::export(module = "dsp")]
pub fn waveform() -> Shared<[f32]> {
    Shared::new(wave().clone())
}

/// Statistics of a signal: a view Rust handed out is read in place, any other array is copied once.
#[svelters::export(module = "dsp")]
pub fn stats(signal: Shared<[f32]>) -> SignalStats {
    let (mut min, mut max, mut sum) = (f64::INFINITY, f64::NEG_INFINITY, 0.0);
    for x in signal.iter().map(|x| *x as f64) {
        min = min.min(x);
        max = max.max(x);
        sum += x;
    }
    let len = signal.len();
    if len == 0 {
        (min, max) = (0.0, 0.0);
    }
    SignalStats { len: len as u32, min, max, mean: if len == 0 { 0.0 } else { sum / len as f64 } }
}

/// True if `view` is `waveform()`'s own memory (no copy was made on the way in).
#[svelters::export(module = "dsp")]
pub fn is_waveform(view: Shared<[f32]>) -> bool {
    Arc::ptr_eq(view.arc(), wave())
}
