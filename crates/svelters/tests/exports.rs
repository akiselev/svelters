//! E0 exports end to end on the log backend: the macros, the schema, the host routing, the built-in
//! `svelters:app/*` modules, identity, dispose, GC, errors, panics, the op log and the typings.

use std::cell::Cell;
use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use svelters_backend_log::LogBackend;
use svelters_host::{Host, testing::fixture_config};

static COUNTER_DROPS: AtomicUsize = AtomicUsize::new(0);
static BOOK_DROPS: AtomicUsize = AtomicUsize::new(0);

/// A point.
#[derive(svelters::Record, Clone, Debug)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    #[js(default = 1.0)]
    pub weight: f64,
    pub label: Option<String>,
    #[js(bigint)]
    pub stamp: i64,
}

#[derive(svelters::Enum, Clone, Copy, Debug)]
pub enum Mode {
    Fast,
    VerySlow,
}

#[derive(svelters::Error, Debug)]
pub enum LibError {
    TooBig(u32),
    #[js(code = "nothing")]
    Empty,
}

impl fmt::Display for LibError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LibError::TooBig(n) => write!(f, "{n} is too big"),
            LibError::Empty => write!(f, "empty"),
        }
    }
}

/// A UI-thread counter.
#[svelters::object(module = "lib")]
pub struct Counter {
    value: i64,
    not_send: Cell<u8>,
}

impl Drop for Counter {
    fn drop(&mut self) {
        COUNTER_DROPS.fetch_add(1, Ordering::SeqCst);
    }
}

#[svelters::export]
impl Counter {
    /// Starts at `start`.
    #[js(constructor)]
    pub fn new(start: i64) -> Self {
        Counter {
            value: start,
            not_send: Cell::new(0),
        }
    }
    #[js(getter)]
    pub fn value(&self) -> i64 {
        self.value
    }
    #[js(setter)]
    pub fn set_value(&mut self, v: i64) {
        self.value = v;
    }
    pub fn add(&mut self, n: i64) -> i64 {
        self.not_send.set(self.not_send.get().wrapping_add(1));
        self.value += n;
        self.value
    }
    pub fn mode_name(&self, m: Mode) -> String {
        format!("{m:?}")
    }
    pub fn merge(&mut self, other: &Counter) {
        self.value += other.value;
    }
    pub fn boom(&mut self) {
        panic!("boom");
    }
    fn private_helper(&self) -> i64 {
        self.value
    }
}

#[svelters::export]
impl Counter {
    #[js(rename = "zero")]
    pub fn make_zero() -> Counter {
        Counter::new(0)
    }
    pub fn twice(&self) -> i64 {
        self.private_helper() * 2
    }
}

#[svelters::object(shared, module = "lib")]
pub struct Book {
    title: String,
}

impl Drop for Book {
    fn drop(&mut self) {
        BOOK_DROPS.fetch_add(1, Ordering::SeqCst);
    }
}

#[svelters::export]
impl Book {
    #[js(getter)]
    pub fn title(&self) -> String {
        self.title.clone()
    }
    pub fn me(self: Arc<Self>) -> Arc<Book> {
        self
    }
}

#[svelters::object(shared, module = "lib")]
pub struct Library {
    books: Mutex<Vec<Arc<Book>>>,
}

#[svelters::export]
impl Library {
    #[js(constructor)]
    pub fn new(titles: Vec<String>) -> Arc<Library> {
        Arc::new(Library {
            books: Mutex::new(
                titles
                    .into_iter()
                    .map(|title| Arc::new(Book { title }))
                    .collect(),
            ),
        })
    }
    pub fn get(&self, i: u32) -> Option<Arc<Book>> {
        self.books.lock().unwrap().get(i as usize).cloned()
    }
    pub fn all(&self) -> Vec<Arc<Book>> {
        self.books.lock().unwrap().clone()
    }
    pub fn take(&self, i: u32) -> Result<Arc<Book>, LibError> {
        let mut b = self.books.lock().unwrap();
        if b.is_empty() {
            return Err(LibError::Empty);
        }
        if i as usize >= b.len() {
            return Err(LibError::TooBig(i));
        }
        Ok(b.remove(i as usize))
    }
}

#[svelters::export(module = "lib")]
pub fn greet(name: &str) -> String {
    format!("hello {name}")
}

#[svelters::export(module = "lib")]
pub fn sum(xs: &[f64]) -> f64 {
    xs.iter().sum()
}

#[svelters::export(module = "lib")]
pub fn scale(p: Point, k: f64) -> Point {
    Point {
        x: p.x * k,
        y: p.y * k,
        ..p
    }
}

#[svelters::export(module = "lib")]
#[js(bigint)]
pub fn big(#[js(bigint)] n: i64) -> i64 {
    n * 2
}

#[svelters::export(module = "lib")]
pub fn unsafe_int(n: i64) -> i64 {
    n * 1024
}

#[svelters::export(module = "other")]
pub fn first_book(lib: Arc<Library>) -> Option<Arc<Book>> {
    lib.get(0)
}

fn stage(name: &str, main: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("svelters-exports-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("main.js"), main).unwrap();
    dir
}

struct Run {
    lines: Vec<String>,
    errors: Vec<String>,
    log: String,
}

fn run(name: &str, main: &str, after: impl FnOnce(&Host<'_>)) -> Run {
    let dir = stage(name, main);
    let (cfg, _) = fixture_config();
    let log = LogBackend::new(Vec::new());
    let vt = log.vtable();
    let host = Host::new(vt.backend_ref(), cfg).unwrap();
    let sink = log.clone();
    host.set_op_log(move |l| sink.append_line(l));
    if let Err(e) = host.load_bundle(dir.join("main.js")) {
        panic!("load: {e}");
    }
    host.checkpoint();
    host.drain_jobs();
    after(&host);
    let lines = host.take_console().into_iter().map(|l| l.text).collect();
    let errors = host
        .take_errors()
        .into_iter()
        .map(|e| e.to_string())
        .collect();
    Run {
        lines,
        errors,
        log: log.log(),
    }
}

fn has(r: &Run, line: &str) {
    assert!(
        r.lines.iter().any(|l| l == line),
        "missing console line {line:?}\nlines: {:#?}\nerrors: {:#?}",
        r.lines,
        r.errors
    );
}

#[test]
fn calls_values_and_errors() {
    let r = run(
        "calls",
        r#"
import { Counter, Library, greet, sum, scale, big, unsafeInt } from 'svelters:app/lib';
const c = new Counter(5);
console.log('add', c.add(2), c.value);
c.value = 40;
console.log('set', c.value, c.twice());
console.log('mode', c.modeName('very-slow'));
try { c.modeName('nope'); } catch (e) { console.log('enum-err', e.name); }
console.log('zero', Counter.zero().value);
const other = new Counter(2);
c.merge(other);
console.log('merge', c.value);
console.log('greet', greet('svelte'));
console.log('sum', sum([1, 2, 3.5]));
const p = scale({ x: 1, y: 2, label: null, stamp: 7n }, 3);
console.log('scale', JSON.stringify({ ...p, stamp: String(p.stamp) }), typeof p.stamp);
console.log('big', big(2n ** 60n) === 2n ** 61n);
try { unsafeInt(2 ** 50); } catch (e) { console.log('range', e.name); }
try { c.add(1.5); } catch (e) { console.log('int', e.name); }
try { c.add(); } catch (e) { console.log('arity', e.name); }
const lib = new Library(['a']);
try { lib.take(5); } catch (e) { console.log('err', e.name, e.domain, e.code, e.message); }
lib.take(0);
try { lib.take(0); } catch (e) { console.log('err', e.code); }
try { c.boom(); } catch (e) { console.log('panic', e.domain, e.code); }
try { c.add(1); } catch (e) { console.log('poisoned', e.code); }
console.log('other', c instanceof Counter, c.nativeType);
"#,
        |_| {},
    );
    assert!(r.errors.is_empty(), "{:#?}", r.errors);
    has(&r, "add 7 7");
    has(&r, "set 40 80");
    has(&r, "mode VerySlow");
    has(&r, "enum-err TypeError");
    has(&r, "zero 0");
    has(&r, "merge 42");
    has(&r, "greet hello svelte");
    has(&r, "sum 6.5");
    has(
        &r,
        r#"scale {"x":3,"y":6,"weight":1,"label":null,"stamp":"7"} bigint"#,
    );
    has(&r, "big true");
    has(&r, "range RangeError");
    has(&r, "int RangeError");
    has(&r, "arity TypeError");
    has(&r, "err NativeError app:lib too-big 5 is too big");
    has(&r, "err nothing");
    has(&r, "panic svelters panic");
    has(&r, "poisoned poisoned");
    has(&r, "other true app:lib.Counter");
    // Decision 3 (2026-10-21): exported calls are in the op log, in full.
    for line in [
        "construct #a1 app:lib.Counter [5]",
        "call #a1 app:lib.Counter.add [2]",
        "set #a1 app:lib.Counter.value 40",
        "static app:lib.Counter.zero [] #a2",
        "static app:lib.greet [\"svelte\"]",
        "call #a1 app:lib.Counter.merge [#a3]",
    ] {
        let _ = line;
    }
    assert!(
        r.log.contains("construct #a1 app:lib.Counter [5]\n"),
        "{}",
        r.log
    );
    assert!(
        r.log.contains("call #a1 app:lib.Counter.add [2]\n"),
        "{}",
        r.log
    );
    assert!(
        r.log.contains("set #a1 app:lib.Counter.value 40\n"),
        "{}",
        r.log
    );
    assert!(
        r.log.contains("static app:lib.Counter.zero [] #a2\n"),
        "{}",
        r.log
    );
    assert!(
        r.log.contains("static app:lib.greet [\"svelte\"]\n"),
        "{}",
        r.log
    );
}

#[test]
fn identity_dispose_and_gc() {
    let books_before = BOOK_DROPS.load(Ordering::SeqCst);
    let counters_before = COUNTER_DROPS.load(Ordering::SeqCst);
    let r = run(
        "identity",
        r#"
import { Counter, Library, Book } from 'svelters:app/lib';
import { firstBook } from 'svelters:app/other';
const lib = new Library(['a', 'b']);
const a1 = lib.get(0), a2 = lib.get(0), b = lib.get(1);
console.log('same', a1 === a2, a1 !== b, a1 instanceof Book, a1.title, b.title);
console.log('arc-self', a1.me() === a1, firstBook(lib) === a1);
console.log('all', lib.all().map((x) => x.title).join(','), lib.all()[1] === b);
console.log('none', lib.get(9));
const c = new Counter(1);
c.dispose();
try { c.add(1); } catch (e) { console.log('disposed', e.name); }
c.dispose();
globalThis.keep = lib;
(() => { for (let i = 0; i < 3; i++) new Counter(i); })();
"#,
        |host| {
            // QuickJS frees unreachable wrappers by refcount at once; a full collection settles the rest.
            host.collect_garbage();
            host.checkpoint();
            host.drain_jobs();
            assert_eq!(
                host.live_app_objects(),
                3,
                "lib + 2 books (the counters were disposed or collected)"
            );
        },
    );
    assert!(r.errors.is_empty(), "{:#?}", r.errors);
    has(&r, "same true true true a b");
    has(&r, "arc-self true true");
    has(&r, "all a,b true");
    has(&r, "none null");
    has(&r, "disposed StaleHandleError");
    assert!(
        COUNTER_DROPS.load(Ordering::SeqCst) - counters_before >= 4,
        "disposed + collected counters dropped in Rust"
    );
    assert_eq!(
        BOOK_DROPS.load(Ordering::SeqCst),
        books_before + 2,
        "dropping the host released the library and its books"
    );
}

#[test]
fn gc_releases_unreachable_objects() {
    let before = COUNTER_DROPS.load(Ordering::SeqCst);
    let dir = stage(
        "gc",
        r#"
import { Counter } from 'svelters:app/lib';
globalThis.kept = new Counter(1);
(() => { for (let i = 0; i < 4; i++) new Counter(i).add(1); })();
"#,
    );
    let (cfg, _) = fixture_config();
    let log = LogBackend::new(Vec::new());
    let vt = log.vtable();
    let host = Host::new(vt.backend_ref(), cfg).unwrap();
    host.load_bundle(dir.join("main.js")).unwrap();
    host.drain_jobs();
    // QuickJS frees most unreachable wrappers by refcount already (their finalizers ran as jobs).
    assert!(host.live_app_objects() <= 5);
    host.collect_garbage();
    host.checkpoint();
    host.drain_jobs();
    assert_eq!(
        host.live_app_objects(),
        1,
        "the collected wrappers released their objects"
    );
    assert!(COUNTER_DROPS.load(Ordering::SeqCst) - before >= 4);
    assert!(host.take_errors().is_empty());
    drop(host);
}

#[test]
fn schema_typings_and_json() {
    let s = svelters::exports::schema();
    let names: Vec<&str> = s.modules.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, ["lib", "other"]);
    let dts = svelters::exports::typescript();
    for want in [
        "declare module 'svelters:app/lib' {",
        "export interface Point { x: number; y: number; weight?: number; label?: string | null; stamp: bigint; }",
        "export type Mode = 'fast' | 'very-slow';",
        "export class Counter extends NativeObject {",
        "/** Starts at `start`. */",
        "constructor(start: number);",
        "value: number;",
        "add(n: number): number;",
        "static zero(): Counter;",
        "readonly title: string;",
        "@throws NativeError `app:lib` `too-big` | `nothing`",
        "get(i: number): Book | null;",
        "export function big(n: bigint): bigint;",
        "export function sum(xs: number[]): number;",
        "export function firstBook(lib: import('svelters:app/lib').Library): import('svelters:app/lib').Book | null;",
    ] {
        assert!(dts.contains(want), "typings lack {want:?}:\n{dts}");
    }
    assert!(!dts.contains("privateHelper"));
    let json: serde_json::Value = serde_json::from_str(&svelters::exports::json()).unwrap();
    assert_eq!(json["schema"], "svelters-exports/1");
    assert_eq!(
        json["modules"][0]["classes"][1]["native"],
        "app:lib.Counter"
    );
    let js =
        svelters::exports::javascript("other", "@svelters/types/core", &|m| format!("./{m}.js"))
            .unwrap();
    assert!(
        js.contains("import { Book as lib$Book, Library as lib$Library } from './lib.js';")
            || js.contains("from './lib.js'"),
        "{js}"
    );
}

#[test]
fn dump_rewrites_only_changes() {
    let dir = std::env::temp_dir().join(format!("svelters-exports-dump-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let first = svelters::exports::dump(&dir).unwrap();
    assert_eq!(first.written.len(), 4, "{first:?}");
    let second = svelters::exports::dump(&dir).unwrap();
    assert!(second.written.is_empty());
    assert_eq!(second.unchanged, 4);
    std::fs::write(dir.join("gone.js"), "").unwrap();
    let third = svelters::exports::dump(&dir).unwrap();
    assert_eq!(third.written, vec![dir.join("gone.js")]);
    assert!(!dir.join("gone.js").exists());
}

#[test]
fn unknown_module_and_member_errors() {
    let r = run(
        "unknown",
        r#"
try { globalThis.__svelters_host.callStatic('app:lib', 'nope', []); } catch (e) { console.log('member', e.name, e.message); }
try { globalThis.__svelters_host.construct('app:nope.X', []); } catch (e) { console.log('kind', e.message.startsWith("unknown kind 'app:nope.X'")); }
import('svelters:app/nope').catch((e) => console.log('module', String(e).includes('no such module')));
"#,
        |_| {},
    );
    has(&r, "member TypeError app:lib has no method 'nope'");
    has(&r, "kind true");
    has(&r, "module true");
}
