//! The Rust side of `examples/exports`: the `svelters:app/demo` module the Svelte app in `app/` imports.

use std::sync::{Arc, Mutex};

/// Where the debug binary writes `exports.json`, `app.d.ts` and the stubs: the app's `.svelters/app`.
pub fn exports_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("app/.svelters/app")
}

/// The app bundle (`pnpm build` in `app/`).
pub fn bundle() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("app/dist/app.js")
}

/// Keeps this crate's registrations linked into binaries that only use it through `svelters:app/*`.
pub fn link() {
    std::hint::black_box(Tally::new as fn(i64) -> Tally);
}

/// How a tally counts.
#[derive(svelters::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    One,
    Ten,
}

/// A summary of a tally.
#[derive(svelters::Record, Clone, Debug)]
pub struct Summary {
    /// The current count.
    pub count: i64,
    /// How many times it was bumped.
    pub bumps: u32,
    pub step: Step,
    pub note: Option<String>,
}

#[derive(svelters::Error, Debug)]
pub enum TallyError {
    /// The count would leave the allowed range.
    OutOfRange(i64),
}

impl std::fmt::Display for TallyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TallyError::OutOfRange(n) => write!(f, "{n} is outside -1000..=1000"),
        }
    }
}

/// A counter that lives on the UI thread (`&mut self`).
#[svelters::object(module = "demo")]
pub struct Tally {
    count: i64,
    bumps: u32,
    step: Step,
}

#[svelters::export]
impl Tally {
    /// A tally starting at `start`.
    #[js(constructor)]
    pub fn new(start: i64) -> Tally {
        Tally { count: start, bumps: 0, step: Step::One }
    }

    /// The current count.
    #[js(getter)]
    pub fn count(&self) -> i64 {
        self.count
    }

    /// One bump (by the current step).
    #[js(getter)]
    pub fn step(&self) -> Step {
        self.step
    }

    #[js(setter)]
    pub fn set_step(&mut self, s: Step) {
        self.step = s;
    }

    /// Adds one step; fails outside -1000..=1000.
    pub fn bump(&mut self) -> Result<i64, TallyError> {
        let by = if self.step == Step::Ten { 10 } else { 1 };
        let next = self.count + by;
        if !(-1000..=1000).contains(&next) {
            return Err(TallyError::OutOfRange(next));
        }
        self.count = next;
        self.bumps += 1;
        Ok(self.count)
    }

    pub fn summary(&self, note: Option<String>) -> Summary {
        Summary { count: self.count, bumps: self.bumps, step: self.step, note }
    }
}

/// A note shared with worker threads later (E1): `Arc<Self>`.
#[svelters::object(shared, module = "demo")]
pub struct Note {
    text: String,
}

#[svelters::export]
impl Note {
    #[js(getter)]
    pub fn text(&self) -> String {
        self.text.clone()
    }
}

/// A notebook of shared notes.
#[svelters::object(shared, module = "demo")]
pub struct Notebook {
    notes: Mutex<Vec<Arc<Note>>>,
}

#[svelters::export]
impl Notebook {
    #[js(constructor)]
    pub fn new() -> Notebook {
        Notebook { notes: Mutex::new(Vec::new()) }
    }

    /// Adds a note and returns it.
    pub fn add(&self, text: &str) -> Arc<Note> {
        let n = Arc::new(Note { text: text.to_string() });
        self.notes.lock().unwrap().push(n.clone());
        n
    }

    /// The note at `i` (the same JS object as the one `add` returned).
    pub fn get(&self, i: u32) -> Option<Arc<Note>> {
        self.notes.lock().unwrap().get(i as usize).cloned()
    }

    #[js(getter)]
    pub fn len(&self) -> u32 {
        self.notes.lock().unwrap().len() as u32
    }
}

/// A greeting from Rust.
#[svelters::export(module = "demo")]
pub fn greet(name: &str) -> String {
    format!("Hello from Rust, {name}!")
}

/// The n-th Fibonacci number (a checked JS number).
#[svelters::export(module = "demo")]
pub fn fib(n: u32) -> u64 {
    let (mut a, mut b) = (0u64, 1u64);
    for _ in 0..n {
        (a, b) = (b, a.wrapping_add(b));
    }
    a
}

/// A no-op, for measuring the per-call cost of the registry path.
#[svelters::export(module = "demo")]
pub fn noop(x: f64) -> f64 {
    x
}

// ---- E1: offload ----

/// What a crunch found.
#[derive(svelters::Record, Clone, Debug)]
pub struct Crunch {
    /// Primes found.
    pub primes: u32,
    /// Wall time of the computation in milliseconds.
    pub ms: f64,
}

/// Counts primes for about `ms` milliseconds of CPU; checks `cx` for cancellation and reports progress.
fn crunch_for(ms: u32, cx: Option<&svelters::WorkCx<f64>>) -> Result<Crunch, svelters::Cancelled> {
    let t0 = std::time::Instant::now();
    let budget = std::time::Duration::from_millis(ms as u64);
    let (mut n, mut primes) = (2u64, 0u32);
    while t0.elapsed() < budget {
        for _ in 0..256 {
            if (2..).take_while(|d| d * d <= n).all(|d| n % d != 0) {
                primes += 1;
            }
            n += 1;
        }
        if let Some(cx) = cx {
            cx.check()?;
            cx.progress((t0.elapsed().as_secs_f64() * 1e3 / ms.max(1) as f64).min(1.0));
        }
    }
    Ok(Crunch { primes, ms: t0.elapsed().as_secs_f64() * 1e3 })
}

/// Counts primes for about `ms` milliseconds on a worker thread: the UI stays responsive.
/// `options.onProgress` receives the fraction done.
#[svelters::export(module = "demo")]
#[js(worker)]
pub fn crunch(ms: u32, cx: svelters::WorkCx<f64>) -> Result<Crunch, svelters::Cancelled> {
    crunch_for(ms, Some(&cx))
}

/// The same computation on the JS thread, synchronously: the UI freezes meanwhile (for comparison).
#[svelters::export(module = "demo")]
pub fn crunch_blocking(ms: u32) -> Crunch {
    crunch_for(ms, None).unwrap_or(Crunch { primes: 0, ms: 0.0 })
}

/// Counts the words of `text`: an async fn on the JS thread that hands the counting to a worker.
#[svelters::export(module = "demo")]
pub async fn word_count(text: String) -> u32 {
    svelters::exports::worker(move || text.split_whitespace().count() as u32).await
}

/// A worker no-op, for measuring the settle cost of the offload path.
#[svelters::export(module = "demo")]
#[js(worker)]
pub fn noop_worker(x: f64) -> f64 {
    x
}

/// A JS-thread async no-op (ready at once), for measuring the eager path.
#[svelters::export(module = "demo")]
pub async fn noop_local(x: f64) -> f64 {
    x
}
