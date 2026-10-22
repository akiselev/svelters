//! TC-EVAL: what quire-core costs as the model of every svelters text field.
//!
//! Models compared:
//! - `egui`: an egui 0.37 `TextEdit`-like field: `String` + `CCursorRange` + `Arc<Mutex<Undoer>>`
//!   (egui's `TextEditState`), the undoer fed `(cursor, text.to_owned())` twice per focused frame
//!   exactly as `text_edit/builder.rs` does (lines 1215 and 1519), word stops from svelters-egui's
//!   `native/words.rs` (FX1, Pango rules).
//! - `quire`: `quire_core::Document` + one `EditorView` (cadatomic's Document/EditorView split),
//!   every edit through `Document::execute`.
//! - `quire-fieldcfg`: the same with an `EditorConfig` trimmed for a field (no bracket, pair or
//!   comment tables), cloned from a template.
//! - memory only: `ropey::Rope`, quire `Text`, and a "lean" quire (`Text` + `Selection` + `History`
//!   + `RecentChanges`), the pieces a `quire-text` field needs without config, tokens, diagnostics.
//!
//! Heap bytes come from a counting global allocator (live bytes delta, not RSS).
//! Times: `Instant` per op (about 20-30 ns of timer overhead included), medians and p99.

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicIsize, AtomicUsize, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use quire_core::change::ChangeSet;
use quire_core::words::{WordClassifier, word_range_at};
use quire_core::{
    Document, EditorCommand, EditorConfig, EditorView, History, Origin, RecentChanges, Selection,
    Text, UndoMode, Version,
};

#[path = "../../../../crates/svelters-egui/src/native/words.rs"]
#[allow(dead_code)]
mod words;
use words::WordRules;

mod undoer;
use undoer::Undoer;

// ---------------------------------------------------------------------------------------------
// Counting allocator

struct Counting;
static LIVE: AtomicIsize = AtomicIsize::new(0);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        LIVE.fetch_add(l.size() as isize, Relaxed);
        ALLOCS.fetch_add(1, Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size() as isize, Relaxed);
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        LIVE.fetch_add(n as isize - l.size() as isize, Relaxed);
        ALLOCS.fetch_add(1, Relaxed);
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn live() -> isize {
    LIVE.load(Relaxed)
}
fn allocs() -> usize {
    ALLOCS.load(Relaxed)
}

/// Heap bytes and allocations per instance: builds `n` and keeps them alive.
fn heap_per<T>(n: usize, mut make: impl FnMut(usize) -> T) -> (f64, f64) {
    let mut v: Vec<T> = Vec::with_capacity(n);
    let (b0, a0) = (live(), allocs());
    for i in 0..n {
        v.push(make(i));
    }
    let (b1, a1) = (live(), allocs());
    black_box(&v);
    drop(v);
    ((b1 - b0) as f64 / n as f64, (a1 - a0) as f64 / n as f64)
}

// ---------------------------------------------------------------------------------------------
// Timing

struct Stats {
    median: f64,
    p99: f64,
}

fn stats(mut ns: Vec<u64>) -> Stats {
    ns.sort_unstable();
    let at = |q: f64| ns[((ns.len() - 1) as f64 * q) as usize] as f64;
    Stats { median: at(0.5), p99: at(0.99) }
}

/// Times `op` alone, `n` times, after an untimed `setup` each time.
fn per_op<S>(n: usize, mut setup: impl FnMut(usize) -> S, mut op: impl FnMut(S)) -> Stats {
    let mut ns = Vec::with_capacity(n);
    for i in 0..n {
        let s = setup(i);
        let t = Instant::now();
        op(s);
        ns.push(t.elapsed().as_nanos() as u64);
    }
    stats(ns)
}

/// `per_op` over one mutable state that both setup and op touch.
fn per_op_on<T, S>(n: usize, st: &mut T, mut setup: impl FnMut(&mut T, usize) -> S, mut op: impl FnMut(&mut T, S)) -> Stats {
    let mut ns = Vec::with_capacity(n);
    for i in 0..n {
        let s = setup(st, i);
        let t = Instant::now();
        op(st, s);
        ns.push(t.elapsed().as_nanos() as u64);
    }
    stats(ns)
}

fn fmt_ns(ns: f64) -> String {
    if ns >= 1_000_000.0 {
        format!("{:.2} ms", ns / 1e6)
    } else if ns >= 10_000.0 {
        format!("{:.1} µs", ns / 1e3)
    } else if ns >= 1_000.0 {
        format!("{:.2} µs", ns / 1e3)
    } else {
        format!("{ns:.0} ns")
    }
}

fn fmt_b(b: f64) -> String {
    if b >= 10_240.0 { format!("{:.1} KiB", b / 1024.0) } else { format!("{b:.0} B") }
}

// ---------------------------------------------------------------------------------------------
// Texts

fn sample(len: usize) -> String {
    const WORDS: &[&str] = &[
        "resistor", "value", "10k", "0603", "tolerance", "1%", "package", "footprint", "net",
        "GND", "clearance", "0.2mm", "layer", "top", "via",
    ];
    let mut s = String::new();
    let mut i = 0;
    while s.len() < len {
        if !s.is_empty() {
            s.push(' ');
        }
        s.push_str(WORDS[i % WORDS.len()]);
        i += 7;
    }
    s.truncate(len);
    s
}

const SIZES: &[(&str, usize)] = &[("empty", 0), ("20 B", 20), ("200 B", 200), ("10 KiB", 10 * 1024)];

// ---------------------------------------------------------------------------------------------
// The egui-like String field

#[derive(Clone, Copy, PartialEq, Debug, Default)]
struct CCursor {
    index: usize,
    prefer_next_row: bool,
}
#[derive(Clone, Copy, PartialEq, Debug, Default)]
struct CCursorRange {
    primary: CCursor,
    secondary: CCursor,
    h_pos: Option<f32>,
}
impl CCursorRange {
    fn one(i: usize) -> Self {
        let c = CCursor { index: i, prefer_next_row: false };
        CCursorRange { primary: c, secondary: c, h_pos: None }
    }
    fn two(a: usize, b: usize) -> Self {
        CCursorRange {
            primary: CCursor { index: b, prefer_next_row: false },
            secondary: CCursor { index: a, prefer_next_row: false },
            h_pos: None,
        }
    }
}

type TeUndoer = Undoer<(CCursorRange, String)>;

struct StrField {
    text: String,
    cursor: CCursorRange,
    undoer: Arc<Mutex<TeUndoer>>,
}

fn byte_of_char(s: &str, c: usize) -> usize {
    s.char_indices().nth(c).map_or(s.len(), |(b, _)| b)
}

impl StrField {
    fn new(s: &str) -> Self {
        let n = s.chars().count();
        StrField {
            text: s.to_owned(),
            cursor: CCursorRange::one(n),
            undoer: Arc::new(Mutex::new(TeUndoer::default())),
        }
    }
    fn feed(&self, t: f64) {
        self.undoer.lock().unwrap().feed_state(t, &(self.cursor, self.text.as_str().to_owned()));
    }
    /// One focused frame: feed, apply the event, feed (egui's `events()`).
    fn frame(&mut self, t: f64, f: impl FnOnce(&mut Self)) {
        self.feed(t);
        f(self);
        self.feed(t);
    }
    fn type_char(&mut self, c: char) {
        let at = self.cursor.primary.index;
        let b = byte_of_char(&self.text, at);
        self.text.insert(b, c);
        self.cursor = CCursorRange::one(at + 1);
    }
    fn delete_word_left(&mut self) {
        let at = self.cursor.primary.index;
        let p = WordRules::Pango.prev(&self.text, at);
        let (a, b) = (byte_of_char(&self.text, p), byte_of_char(&self.text, at));
        self.text.replace_range(a..b, "");
        self.cursor = CCursorRange::one(p);
    }
    fn select_word(&mut self, at: usize) {
        let end = WordRules::Pango.next(&self.text, at);
        let start = WordRules::Pango.prev(&self.text, end);
        self.cursor = CCursorRange::two(start, end);
    }
    fn undo(&mut self) -> bool {
        let cur = (self.cursor, self.text.clone());
        let r = self.undoer.lock().unwrap().undo(&cur).cloned();
        match r {
            Some((c, t)) => {
                self.cursor = c;
                self.text = t;
                true
            }
            None => false,
        }
    }
    fn redo(&mut self) -> bool {
        let cur = (self.cursor, self.text.clone());
        let r = self.undoer.lock().unwrap().redo(&cur).cloned();
        match r {
            Some((c, t)) => {
                self.cursor = c;
                self.text = t;
                true
            }
            None => false,
        }
    }
    fn set_text(&mut self, s: &str) {
        if self.text != s {
            self.text.clear();
            self.text.push_str(s);
            let n = self.text.chars().count();
            self.cursor.primary.index = self.cursor.primary.index.min(n);
            self.cursor.secondary.index = self.cursor.secondary.index.min(n);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The quire field: Document + one EditorView

struct QField {
    doc: Document,
    view: EditorView,
}

fn field_config() -> EditorConfig {
    EditorConfig {
        brackets: Vec::new(),
        auto_closing_pairs: Vec::new(),
        surrounding_pairs: Vec::new(),
        line_comment: None,
        block_comment: None,
        auto_indent: false,
        ..EditorConfig::default()
    }
}

impl QField {
    fn new(s: &str, cfg: EditorConfig) -> Self {
        let doc = Document::new(s, cfg);
        let mut view = EditorView::new(&doc);
        view.set_selection(&doc, Selection::point(doc.text().len_bytes()));
        QField { doc, view }
    }
    fn exec(&mut self, c: &EditorCommand, t: u64) {
        let _ = black_box(self.doc.execute(&mut self.view, c, Some(t)));
    }
    fn select_word(&mut self, at: usize) {
        // egui_quire glue::word_range_at, then a SetSelection.
        let cls = WordClassifier::new(&self.doc.config().word_separators);
        let r = word_range_at(self.doc.text(), at, &cls);
        let c = EditorCommand::SetSelection { selection: Selection::single(r.start, r.end) };
        let _ = black_box(self.doc.execute(&mut self.view, &c, None));
    }
    fn set_text_remote(&mut self, s: &str) {
        let len = self.doc.text().len_bytes();
        let cs = ChangeSet::replace(len, [(0..len, s)]).unwrap();
        let _ = self.doc.apply_remote(self.doc.version(), cs, Origin::Reload);
        self.view.sync(&self.doc);
    }
    fn set_text_diff(&mut self, s: &str) {
        let cs = ChangeSet::diff(self.doc.text(), &Text::from(s));
        let _ = self.doc.apply_remote(self.doc.version(), cs, Origin::Reload);
        self.view.sync(&self.doc);
    }
}

fn ty(c: &str) -> EditorCommand {
    EditorCommand::Type { text: c.to_string() }
}

// ---------------------------------------------------------------------------------------------

fn main() {
    let only = std::env::args().nth(1);
    let run = |name: &str| only.as_deref().is_none_or(|o| name.contains(o));
    if only.as_deref() == Some("hot") {
        // For a profiler: quire's per-command path on a 20 B field, alone.
        let (a, right) = (ty("a"), EditorCommand::CursorMove { to: quire_core::Motion::Right, select: false });
        let base = sample(20);
        let mut t = 0u64;
        for _ in 0..20_000 {
            let mut q = QField::new(&base, field_config());
            for i in 0..32 {
                t += 30;
                q.exec(if i % 2 == 0 { &a } else { &right }, t);
            }
        }
        return;
    }
    println!("# TC-EVAL results\n");
    if run("memory") {
        memory();
    }
    if run("create") {
        create();
    }
    if run("keys") {
        keys();
    }
    if run("allocs") {
        alloc_probe();
    }
    if run("undo") {
        undo();
    }
    if run("history") {
        history_memory();
    }
    if run("settext") {
        set_text();
    }
    if run("read") {
        read_back();
    }
    if run("grid") {
        grid();
    }
}

fn memory() {
    println!("## Memory per field (heap bytes / allocations, plus inline size)\n");
    println!(
        "inline sizes: String {} B, StrField {} B, Rope {} B, Text {} B, Document {} B, EditorView {} B, EditorConfig {} B\n",
        size_of::<String>(),
        size_of::<StrField>(),
        size_of::<ropey::Rope>(),
        size_of::<Text>(),
        size_of::<Document>(),
        size_of::<EditorView>(),
        size_of::<EditorConfig>()
    );
    println!(
        "lean pieces inline: Selection {} B, History {} B, RecentChanges {} B, Version {} B\n",
        size_of::<Selection>(),
        size_of::<History>(),
        size_of::<RecentChanges>(),
        size_of::<Version>()
    );
    let (cfg_b, cfg_a) = heap_per(1000, |_| EditorConfig::default());
    let (fcfg_b, fcfg_a) = {
        let t = field_config();
        heap_per(1000, |_| t.clone())
    };
    println!("EditorConfig::default(): {} in {cfg_a:.0} allocs; field config clone: {} in {fcfg_a:.0} allocs\n", fmt_b(cfg_b), fmt_b(fcfg_b));
    println!("| model | empty | 20 B | 200 B | 10 KiB |");
    println!("|---|---|---|---|---|");
    let texts: Vec<String> = SIZES.iter().map(|&(_, n)| sample(n)).collect();
    let n = 1000;
    let row = |name: &str, f: &dyn Fn(&str) -> (f64, f64)| {
        let cells: Vec<String> = texts
            .iter()
            .map(|t| {
                let (b, a) = f(t);
                format!("{} / {a:.0}", fmt_b(b))
            })
            .collect();
        println!("| {name} | {} |", cells.join(" | "));
    };
    row("String", &|t| heap_per(n, |_| t.to_owned()));
    row("egui field (String + state), idle", &|t| heap_per(n, |_| StrField::new(t)));
    row("egui field after 1 focused frame", &|t| {
        heap_per(n, |_| {
            let f = StrField::new(t);
            f.feed(0.0);
            f
        })
    });
    row("egui field after 1 keystroke", &|t| {
        heap_per(n, |_| {
            let mut f = StrField::new(t);
            f.frame(0.0, |f| f.type_char('x'));
            f
        })
    });
    row("ropey::Rope", &|t| heap_per(n, |_| ropey::Rope::from_str(t)));
    row("quire Text", &|t| heap_per(n, |_| Text::from(t)));
    row("quire lean (Text+Selection+History+RecentChanges)", &|t| {
        heap_per(n, |_| {
            (Text::from(t), Selection::point(0), History::new(UndoMode::Local), RecentChanges::new(64, Version::INITIAL))
        })
    });
    row("quire Document+EditorView (default config)", &|t| heap_per(n, |_| QField::new(t, EditorConfig::default())));
    let fc = field_config();
    row("quire Document+EditorView (field config)", &|t| heap_per(n, |_| QField::new(t, fc.clone())));
    row("quire D+V (field config) after 1 keystroke", &|t| {
        let c = ty("x");
        heap_per(n, |_| {
            let mut q = QField::new(t, fc.clone());
            q.exec(&c, 0);
            q
        })
    });
    println!();
}

fn create() {
    println!("## Creating 10,000 fields (ms, median of 5)\n");
    println!("| model | empty | 20 B | 200 B | 10 KiB |");
    println!("|---|---|---|---|---|");
    let texts: Vec<String> = SIZES.iter().map(|&(_, n)| sample(n)).collect();
    let time = |f: &dyn Fn(&str)| -> Vec<String> {
        texts
            .iter()
            .map(|t| {
                let mut v: Vec<u64> = (0..5)
                    .map(|_| {
                        let s = Instant::now();
                        f(t);
                        s.elapsed().as_nanos() as u64
                    })
                    .collect();
                v.sort();
                format!("{:.2}", v[2] as f64 / 1e6)
            })
            .collect()
    };
    let row = |name: &str, c: Vec<String>| println!("| {name} | {} |", c.join(" | "));
    row("String", time(&|t| {
        let v: Vec<String> = (0..10_000).map(|_| t.to_owned()).collect();
        black_box(v);
    }));
    row("egui field", time(&|t| {
        let v: Vec<StrField> = (0..10_000).map(|_| StrField::new(t)).collect();
        black_box(v);
    }));
    row("quire Text", time(&|t| {
        let v: Vec<Text> = (0..10_000).map(|_| Text::from(t)).collect();
        black_box(v);
    }));
    row("quire D+V default config", time(&|t| {
        let v: Vec<QField> = (0..10_000).map(|_| QField::new(t, EditorConfig::default())).collect();
        black_box(v);
    }));
    let fc = field_config();
    row("quire D+V field config", time(&|t| {
        let v: Vec<QField> = (0..10_000).map(|_| QField::new(t, fc.clone())).collect();
        black_box(v);
    }));
    println!("\n(times include dropping the 10,000 fields)\n");
}

fn keys() {
    println!("## Per keystroke (median / p99), one op per focused frame, caret at the end\n");
    println!("The field is rebuilt (untimed) every {RESET} ops so its size stays near the nominal one.\n");
    println!("| op | model | 20 B | 200 B | 10 KiB |");
    println!("|---|---|---|---|---|");
    let sizes = &SIZES[1..];
    let n = 4000;
    let line = |op: &str, model: &str, cells: Vec<Stats>| {
        let c: Vec<String> = cells.iter().map(|s| format!("{} / {}", fmt_ns(s.median), fmt_ns(s.p99))).collect();
        println!("| {op} | {model} | {} |", c.join(" | "));
    };
    // egui: one focused frame per op (feed, op, feed); `prep` runs untimed before it.
    let egui = |prep: &dyn Fn(&mut StrField, f64), op: &dyn Fn(&mut StrField, usize)| -> Vec<Stats> {
        sizes.iter().map(|&(_, l)| {
            let base = sample(l);
            let mut f = StrField::new(&base);
            let mut t = 0.0;
            per_op_on(n, &mut f, |f, i| {
                if i % RESET == 0 { *f = StrField::new(&base); f.feed(0.0); }
                t += 0.03;
                prep(f, t);
                t += 0.03;
                (t, i)
            }, |f, (t, i)| f.frame(t, |f| op(f, i)))
        }).collect()
    };
    let quire = |prep: &dyn Fn(&mut QField, u64), op: &dyn Fn(&mut QField, usize, u64)| -> Vec<Stats> {
        sizes.iter().map(|&(_, l)| {
            let base = sample(l);
            let mut q = QField::new(&base, field_config());
            let mut t = 0u64;
            per_op_on(n, &mut q, |q, i| {
                if i % RESET == 0 { *q = QField::new(&base, field_config()); }
                t += 30;
                prep(q, t);
                t += 30;
                (t, i)
            }, |q, (t, i)| op(q, i, t))
        }).collect()
    };
    let (a, sp, right, w, d) = (ty("a"), ty(" "), EditorCommand::CursorMove { to: quire_core::Motion::Right, select: false }, ty(" word"), EditorCommand::DeleteWordLeft);
    line("caret right (command floor)", "egui", egui(&|f, _| f.cursor = CCursorRange::one(0), &|f, _| { let i = f.cursor.primary.index + 1; f.cursor = CCursorRange::one(i) }));
    line("caret right (command floor)", "quire", quire(&|q, _| q.view.set_selection(&q.doc, Selection::point(0)), &|q, _, t| q.exec(&right, t)));
    line("insert char", "egui", egui(&|_, _| {}, &|f, i| f.type_char(if i % 6 == 5 { ' ' } else { 'a' })));
    line("insert char", "quire", quire(&|_, _| {}, &|q, i, t| q.exec(if i % 6 == 5 { &sp } else { &a }, t)));
    line("insert char, no undo feed", "String", sizes.iter().map(|&(_, l)| {
        let base = sample(l);
        let mut f = StrField::new(&base);
        per_op_on(n, &mut f, |f, i| if i % RESET == 0 { *f = StrField::new(&base) }, |f, _| f.type_char('a'))
    }).collect());
    line("delete word left", "egui", egui(&|f, t| f.frame(t, |f| for c in " word".chars() { f.type_char(c) }), &|f, _| f.delete_word_left()));
    line("delete word left", "quire", quire(&|q, t| q.exec(&w, t), &|q, _, t| q.exec(&d, t)));
    line("select word (double click)", "egui", egui(&|_, _| {}, &|f, i| { let n = f.text.chars().count(); f.select_word((i * 7919) % n) }));
    line("select word (double click)", "quire", quire(&|_, _| {}, &|q, i, _| { let t = q.doc.text(); let at = t.clamp_to_boundary((i * 7919) % t.len_bytes().max(1)); q.select_word(at) }));
    // focused frame with no input: egui clones the text twice; quire ticks the undo clock.
    line("idle focused frame", "egui", sizes.iter().map(|&(_, l)| {
        let f = StrField::new(&sample(l));
        f.feed(0.0);
        let mut t = 0.0;
        per_op(n, |_| { t += 0.016; t }, |t| { f.feed(t); f.feed(t) })
    }).collect());
    line("idle focused frame", "quire", sizes.iter().map(|&(_, l)| {
        let mut q = QField::new(&sample(l), field_config());
        let mut t = 0u64;
        per_op_on(n, &mut q, |_, _| { t += 16; t }, |q, t| { black_box(q.doc.tick(t)); black_box(q.view.sync(&q.doc)); })
    }).collect());
    println!();
}

const RESET: usize = 32;

/// Heap allocations per op on a 20 B field (where the per-command floor comes from).
fn alloc_probe() {
    println!("## Allocations per op, 20 B field\n");
    println!("| op | egui (incl. 2 undo feeds) | quire |");
    println!("|---|---|---|");
    let base = sample(20);
    let count = |f: &mut dyn FnMut()| { let a = allocs(); f(); allocs() - a };
    let (a, right, w, d) = (ty("a"), EditorCommand::CursorMove { to: quire_core::Motion::Right, select: false }, ty(" word"), EditorCommand::DeleteWordLeft);
    let mut q = QField::new(&base, field_config());
    let mut f = StrField::new(&base);
    f.feed(0.0);
    q.exec(&a, 0);
    f.frame(0.03, |f| f.type_char('a'));
    let rows: [(&str, Box<dyn Fn(&mut StrField)>, Box<dyn Fn(&mut QField)>); 4] = [
        ("caret right", Box::new(|f| { let i = f.cursor.primary.index; f.frame(1.0, |f| f.cursor = CCursorRange::one(i.saturating_sub(1))) }), Box::new(move |q| q.exec(&right, 100))),
        ("insert char", Box::new(|f| f.frame(1.03, |f| f.type_char('b'))), Box::new(move |q| q.exec(&a, 130))),
        ("delete word left (after typing \" word\")", Box::new(|f| { f.frame(1.06, |f| for c in " word".chars() { f.type_char(c) }); }), Box::new(move |q| q.exec(&w, 160))),
        ("", Box::new(|f| f.frame(1.09, |f| f.delete_word_left())), Box::new(move |q| q.exec(&d, 190))),
    ];
    let mut out = Vec::new();
    for (name, e, qq) in rows.iter() {
        let ea = count(&mut || e(&mut f));
        let qa = count(&mut || qq(&mut q));
        out.push((name.to_string(), ea, qa));
    }
    out[3].0 = "delete word left".into();
    for (name, ea, qa) in out.iter().filter(|r| !r.0.starts_with("delete word left (")) {
        println!("| {name} | {ea} | {qa} |");
    }
    let mut q = QField::new(&base, field_config());
    let c = count(&mut || q.select_word(3));
    let c2 = count(&mut || { let cls = WordClassifier::new(&q.doc.config().word_separators); black_box(word_range_at(q.doc.text(), 3, &cls)); });
    let mut f = StrField::new(&base);
    let e = count(&mut || f.frame(2.0, |f| f.select_word(3)));
    println!("| select word | {e} | {c} (word_range_at alone: {c2}) |");
    println!();
}

const TYPED: &str = "the quick brown fox jumps over the lazy dog and keeps running far away from the farm yard ok done";

fn undo() {
    println!("## Undo/redo of 100 typed chars (30 ms apart, typed at the end)\n");
    println!("| model | size | type 100 (total) | undo steps | undo all (total) | redo all (total) |");
    println!("|---|---|---|---|---|---|");
    let typed: String = format!("{TYPED}{}", "!".repeat(100 - TYPED.len()));
    for &(name, l) in &SIZES[1..] {
        let base = sample(l);
        // egui
        let mut ts = Vec::new();
        let mut steps = 0;
        let (mut tu, mut tr) = (Vec::new(), Vec::new());
        for _ in 0..7 {
            let mut f = StrField::new(&base);
            f.feed(0.0);
            let mut t = 0.0;
            let s = Instant::now();
            for c in typed.chars() {
                t += 0.03;
                f.frame(t, |f| f.type_char(c));
            }
            ts.push(s.elapsed().as_nanos() as u64);
            // frames after typing stops, until the undoer settles (stable_time = 1 s)
            for _ in 0..80 {
                t += 0.016;
                f.feed(t);
            }
            let s = Instant::now();
            steps = 0;
            while f.undo() {
                steps += 1;
            }
            tu.push(s.elapsed().as_nanos() as u64);
            assert_eq!(f.text, base);
            let s = Instant::now();
            while f.redo() {}
            tr.push(s.elapsed().as_nanos() as u64);
        }
        println!("| egui | {name} | {} | {steps} | {} | {} |", fmt_ns(stats(ts).median), fmt_ns(stats(tu).median), fmt_ns(stats(tr).median));
        // quire
        let mut ts = Vec::new();
        let (mut tu, mut tr) = (Vec::new(), Vec::new());
        let cmds: Vec<EditorCommand> = typed.chars().map(|c| ty(&c.to_string())).collect();
        for _ in 0..7 {
            let mut q = QField::new(&base, field_config());
            let mut t = 0u64;
            let s = Instant::now();
            for c in &cmds {
                t += 30;
                q.exec(c, t);
            }
            ts.push(s.elapsed().as_nanos() as u64);
            q.doc.close_undo_group();
            let s = Instant::now();
            steps = 0;
            while q.doc.history().can_undo() {
                q.exec(&EditorCommand::Undo, t);
                steps += 1;
            }
            tu.push(s.elapsed().as_nanos() as u64);
            assert!(*q.doc.text() == *base.as_str());
            let s = Instant::now();
            while q.doc.history().can_redo() {
                q.exec(&EditorCommand::Redo, t);
            }
            tr.push(s.elapsed().as_nanos() as u64);
            assert!(*q.doc.text() == *format!("{base}{typed}").as_str());
        }
        println!("| quire | {name} | {} | {steps} | {} | {} |", fmt_ns(stats(ts).median), fmt_ns(stats(tu).median), fmt_ns(stats(tr).median));
    }
    println!();
}

/// History memory: 20 words typed with a 1.5 s pause after each (20 undo groups in both models).
fn history_memory() {
    println!("## History memory after 20 undo groups (20 words, 1.5 s pause after each)\n");
    println!("| model | 20 B | 200 B | 10 KiB |");
    println!("|---|---|---|---|");
    let words: Vec<&str> = TYPED.split(' ').take(20).collect();
    let mut cells_e = Vec::new();
    let mut cells_q = Vec::new();
    let mut cells_q4 = Vec::new();
    for &(_, l) in &SIZES[1..] {
        let base = sample(l);
        let f0 = live();
        let mut f = StrField::new(&base);
        f.feed(0.0);
        let mut t = 0.0;
        for w in &words {
            for c in format!(" {w}").chars() {
                t += 0.03;
                f.frame(t, |f| f.type_char(c));
            }
            for _ in 0..100 {
                t += 0.016;
                f.feed(t);
            }
        }
        cells_e.push(fmt_b((live() - f0) as f64));
        black_box(&f);
        drop(f);
        let q0 = live();
        let mut q = QField::new(&base, field_config());
        let mut tm = 0u64;
        for w in &words {
            for c in format!(" {w}").chars() {
                tm += 30;
                q.exec(&ty(&c.to_string()), tm);
            }
            tm += 1500;
            q.doc.tick(tm);
        }
        cells_q.push(fmt_b((live() - q0) as f64));
        black_box(&q);
        drop(q);
        let q0 = live();
        let mut q = QField::new(&base, EditorConfig { recent_changes_window: 4, ..field_config() });
        let mut tm = 0u64;
        for w in &words {
            for c in format!(" {w}").chars() {
                tm += 30;
                q.exec(&ty(&c.to_string()), tm);
            }
            tm += 1500;
            q.doc.tick(tm);
        }
        cells_q4.push(fmt_b((live() - q0) as f64));
        black_box(&q);
    }
    println!("| egui (snapshots of the whole text) | {} |", cells_e.join(" | "));
    println!("| quire D+V (change sets), recent_changes_window 64 (default) | {} |", cells_q.join(" | "));
    println!("| quire D+V, recent_changes_window 4 | {} |", cells_q4.join(" | "));
    println!("\n(whole field: text + state + history)\n");
}

fn set_text() {
    println!("## set-text from a prop (bind:value), alternating two values\n");
    println!("| model / method | 20 B: one char changed | 20 B: all new | 200 B: one char | 10 KiB: one char | 10 KiB: all new |");
    println!("|---|---|---|---|---|---|");
    let pairs = |l: usize, all: bool| -> (String, String) {
        let a = sample(l);
        let b = if all {
            a.chars().rev().collect()
        } else {
            let mut b = a.clone().into_bytes();
            let m = b.len() / 2;
            b[m] = if b[m] == b'Z' { b'Y' } else { b'Z' };
            String::from_utf8(b).unwrap()
        };
        (a, b)
    };
    let cases = [(20, false), (20, true), (200, false), (10 * 1024, false), (10 * 1024, true)];
    let n = 2000;
    let cell = |s: Stats| format!("{} / {}", fmt_ns(s.median), fmt_ns(s.p99));
    let row = |name: &str, f: &dyn Fn(&str, &str) -> Stats| {
        let c: Vec<String> = cases.iter().map(|&(l, all)| { let (a, b) = pairs(l, all); cell(f(&a, &b)) }).collect();
        println!("| {name} | {} |", c.join(" | "));
    };
    row("String: compare + replace", &|a, b| {
        let mut f = StrField::new(a);
        per_op(n, |i| if i % 2 == 0 { b } else { a }, |s| f.set_text(s))
    });
    row("quire: replace-all ChangeSet, apply_remote(Reload)", &|a, b| {
        let mut q = QField::new(a, field_config());
        per_op(n, |i| if i % 2 == 0 { b } else { a }, |s| q.set_text_remote(s))
    });
    row("quire: ChangeSet::diff + apply_remote(Reload)", &|a, b| {
        let mut q = QField::new(a, field_config());
        per_op(n, |i| if i % 2 == 0 { b } else { a }, |s| q.set_text_diff(s))
    });
    row("quire: rebuild Document + EditorView", &|a, b| {
        let fc = field_config();
        let mut q = QField::new(a, fc.clone());
        per_op(n, |i| if i % 2 == 0 { b } else { a }, |s| { q = QField::new(s, fc.clone()); })
    });
    row("quire: compare only (Text == &str), unchanged", &|a, _| {
        let q = QField::new(a, field_config());
        per_op(n, |_| a, |s| { black_box(*q.doc.text() == *s); })
    });
    row("String: compare only, unchanged", &|a, _| {
        let f = StrField::new(a);
        per_op(n, |_| a, |s| { black_box(f.text == s); })
    });
    println!();
}

fn read_back() {
    println!("## Reading the text back each frame\n");
    println!("| read | 20 B | 200 B | 10 KiB |");
    println!("|---|---|---|---|");
    let n = 4000;
    let cell = |s: Stats| format!("{} / {}", fmt_ns(s.median), fmt_ns(s.p99));
    let row = |name: &str, f: &dyn Fn(&str) -> Stats| {
        let c: Vec<String> = SIZES[1..].iter().map(|&(_, l)| cell(f(&sample(l)))).collect();
        println!("| {name} | {} |", c.join(" | "));
    };
    row("String: as_str", &|s| { let f = StrField::new(s); per_op(n, |_| (), |_| { black_box(f.text.as_str()); }) });
    row("String: to_owned (LayoutJob copy)", &|s| { let f = StrField::new(s); per_op(n, |_| (), |_| { black_box(f.text.to_owned()); }) });
    row("quire: as_slice().as_str() (Some if one chunk)", &|s| { let q = QField::new(s, field_config()); per_op(n, |_| (), |_| { black_box(q.doc.text().as_slice().as_str()); }) });
    row("quire: to_cow()", &|s| { let q = QField::new(s, field_config()); per_op(n, |_| (), |_| { black_box(q.doc.text().as_slice().to_cow()); }) });
    row("quire: to_string()", &|s| { let q = QField::new(s, field_config()); per_op(n, |_| (), |_| { black_box(q.doc.text().to_string()); }) });
    row("quire after typing 100 chars: to_cow()", &|s| {
        let mut q = QField::new(s, field_config());
        for (i, c) in TYPED.chars().enumerate() { q.exec(&ty(&c.to_string()), i as u64 * 30); }
        per_op(n, |_| (), |_| { black_box(q.doc.text().as_slice().to_cow()); })
    });
    let contiguous: Vec<String> = SIZES[1..].iter().map(|&(_, l)| {
        let q = QField::new(&sample(l), field_config());
        let mut q2 = QField::new(&sample(l), field_config());
        for (i, c) in TYPED.chars().enumerate() { q2.exec(&ty(&c.to_string()), i as u64 * 30); }
        format!("{} / {}", q.doc.text().as_slice().as_str().is_some(), q2.doc.text().as_slice().as_str().is_some())
    }).collect();
    println!("| one chunk? (fresh / after typing) | {} |", contiguous.join(" | "));
    println!();
}

/// A 10,000-row PropertyGrid: every row's value is read and compared with its prop each frame
/// (the render pass), three rows are focused and get one keystroke per frame.
fn grid() {
    println!("## PropertyGrid: 10,000 rows, 3 focused (one keystroke each per frame)\n");
    println!("| model | heap, 10k rows | build 10k | frame: idle rows (read + compare) | frame: 3 focused (edit + undo upkeep) | frame total |");
    println!("|---|---|---|---|---|---|");
    let rows = 10_000;
    let values: Vec<String> = (0..rows)
        .map(|i| match i % 4 {
            0 => format!("R{i} 10k 0603 1%"),
            1 => format!("{:.3} mm", i as f64 * 0.127),
            2 => String::new(),
            _ => format!("net_{i}_GND"),
        })
        .collect();
    let focused = [17usize, 4242, 9001];
    let frames = 200;
    let fnv = |s: &[u8]| s.iter().fold(0xcbf29ce484222325u64, |h, b| (h ^ *b as u64).wrapping_mul(0x100000001b3));

    // egui String fields
    {
        let b0 = live();
        let s = Instant::now();
        let mut v: Vec<StrField> = values.iter().map(|t| StrField::new(t)).collect();
        let build = s.elapsed();
        let heap = live() - b0;
        let (mut idle, mut foc) = (Vec::new(), Vec::new());
        for fr in 0..frames {
            let t = fr as f64 * 0.016;
            let s = Instant::now();
            for (i, f) in v.iter().enumerate() {
                if !focused.contains(&i) {
                    black_box(fnv(f.text.as_bytes()));
                    black_box(f.text == values[i]);
                }
            }
            idle.push(s.elapsed().as_nanos() as u64);
            let s = Instant::now();
            for &i in &focused {
                v[i].frame(t, |f| f.type_char('a'));
                black_box(fnv(v[i].text.as_bytes()));
            }
            foc.push(s.elapsed().as_nanos() as u64);
        }
        let (i, f) = (stats(idle).median, stats(foc).median);
        println!("| egui field | {} | {:.2} ms | {} | {} | {} |", fmt_b(heap as f64), build.as_secs_f64() * 1e3, fmt_ns(i), fmt_ns(f), fmt_ns(i + f));
        black_box(v);
    }
    // quire everywhere
    for (name, cfg) in [("quire D+V, default config", EditorConfig::default()), ("quire D+V, field config", field_config())] {
        let b0 = live();
        let s = Instant::now();
        let mut v: Vec<QField> = values.iter().map(|t| QField::new(t, cfg.clone())).collect();
        let build = s.elapsed();
        let heap = live() - b0;
        let a = ty("a");
        let (mut idle, mut foc) = (Vec::new(), Vec::new());
        for fr in 0..frames {
            let t = fr as u64 * 16;
            let s = Instant::now();
            for (i, q) in v.iter().enumerate() {
                if !focused.contains(&i) {
                    let c = q.doc.text().as_slice().to_cow();
                    black_box(fnv(c.as_bytes()));
                    black_box(*q.doc.text() == *values[i].as_str());
                }
            }
            idle.push(s.elapsed().as_nanos() as u64);
            let s = Instant::now();
            for &i in &focused {
                v[i].exec(&a, t);
                v[i].doc.tick(t);
                let c = v[i].doc.text().as_slice().to_cow();
                black_box(fnv(c.as_bytes()));
            }
            foc.push(s.elapsed().as_nanos() as u64);
        }
        let (i, f) = (stats(idle).median, stats(foc).median);
        println!("| {name} | {} | {:.2} ms | {} | {} | {} |", fmt_b(heap as f64), build.as_secs_f64() * 1e3, fmt_ns(i), fmt_ns(f), fmt_ns(i + f));
        black_box(v);
    }
    // lazy hybrid: String until focused, then promoted to Document + EditorView
    {
        enum Hy {
            Idle(String),
            Live(Box<QField>),
        }
        let cfg = field_config();
        let b0 = live();
        let s = Instant::now();
        let mut v: Vec<Hy> = values.iter().map(|t| Hy::Idle(t.clone())).collect();
        let build = s.elapsed();
        let s = Instant::now();
        for &i in &focused {
            if let Hy::Idle(t) = &v[i] {
                v[i] = Hy::Live(Box::new(QField::new(t, cfg.clone())));
            }
        }
        let promote = s.elapsed();
        let heap = live() - b0;
        let a = ty("a");
        let (mut idle, mut foc) = (Vec::new(), Vec::new());
        for fr in 0..frames {
            let t = fr as u64 * 16;
            let s = Instant::now();
            for (i, h) in v.iter().enumerate() {
                if let Hy::Idle(s) = h {
                    black_box(fnv(s.as_bytes()));
                    black_box(*s == values[i]);
                }
            }
            idle.push(s.elapsed().as_nanos() as u64);
            let s = Instant::now();
            for &i in &focused {
                if let Hy::Live(q) = &mut v[i] {
                    q.exec(&a, t);
                    q.doc.tick(t);
                    let c = q.doc.text().as_slice().to_cow();
                    black_box(fnv(c.as_bytes()));
                }
            }
            foc.push(s.elapsed().as_nanos() as u64);
        }
        let (i, f) = (stats(idle).median, stats(foc).median);
        println!("| hybrid (String idle, 3 promoted) | {} | {:.2} ms (+{} to promote 3) | {} | {} | {} |", fmt_b(heap as f64), build.as_secs_f64() * 1e3, fmt_ns(promote.as_nanos() as f64), fmt_ns(i), fmt_ns(f), fmt_ns(i + f));
        black_box(v);
    }
    println!();
}
