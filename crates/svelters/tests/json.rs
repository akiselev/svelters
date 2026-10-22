//! RA2 on the log backend: serde values (`Json<T>`) and `#[js(tag)]` data enums round-trip through
//! export calls (Rust → JS → Rust), bad values give typed `NativeError`s with a path, and the typings
//! and `exports.json` come from the same `&Schema` (SV24).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use svelters::Json;
use svelters_backend_log::LogBackend;
use svelters_host::{Host, testing::fixture_config};

// ---- serde types (a headless crate's DTOs: no svelters derive) ----

/// How bad a refusal is.
#[derive(Serialize, Deserialize, JsonSchema, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Error,
}

/// A structured refusal, like cadatomic's `Refusal`.
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
pub struct Refusal {
    /// The reason id (`args.missing`).
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub data: serde_json::Value,
    pub severity: Severity,
    pub attempts: u8,
}

/// A slot's state (serde's internally tagged form).
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SlotStatus {
    Empty,
    Filled { value: f64, unit: Option<String> },
    Refused(Refusal),
}

fn refusal() -> Refusal {
    Refusal {
        reason: "args.missing".into(),
        slot: Some("face".into()),
        message: "pick a face".into(),
        data: serde_json::json!({ "missing": ["face"] }),
        severity: Severity::Error,
        attempts: 2,
    }
}

#[svelters::export(module = "intent")]
pub fn echo_refusal(r: Json<Refusal>) -> Json<Refusal> {
    r
}

#[svelters::export(module = "intent")]
pub fn refuse(reason: String) -> Json<Refusal> {
    Json(Refusal { reason, ..refusal() })
}

/// What Rust read, as Rust's `Debug`.
#[svelters::export(module = "intent")]
pub fn describe_status(s: Json<SlotStatus>) -> String {
    match s.0 {
        SlotStatus::Empty => "empty".into(),
        SlotStatus::Filled { value, unit } => format!("filled {value} {}", unit.unwrap_or_default()),
        SlotStatus::Refused(r) => format!("refused {} {:?} {}", r.reason, r.severity, r.attempts),
    }
}

#[svelters::export(module = "intent")]
pub fn statuses() -> Vec<Json<SlotStatus>> {
    vec![Json(SlotStatus::Empty), Json(SlotStatus::Filled { value: 1.5, unit: None }), Json(SlotStatus::Refused(refusal()))]
}

#[svelters::export(module = "intent")]
pub fn same_status(s: Json<SlotStatus>) -> Json<SlotStatus> {
    s
}

// ---- SV48: serde's buffered forms (cadatomic's command-path shapes) ----

/// How far an extrusion goes: internally tagged, inside another internally tagged enum.
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum Extent {
    Blind { depth: f64 },
    Through,
    UpTo { face: String, offset: Option<f64> },
}

/// A command body, like cadatomic's `command/body.rs` (tag `kind`, `deny_unknown_fields`).
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Body {
    Extrude { extent: Extent, count: u8 },
    Hole { r: f64, at: Vec<[f64; 2]> },
}

/// Untagged.
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum Amount {
    Value(f64),
    Expr { expr: String },
}

/// Adjacently tagged.
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
#[serde(tag = "t", content = "c", rename_all = "snake_case")]
pub enum Setting {
    Toggle(bool),
    Range { min: f64, max: f64 },
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
pub struct Pos {
    pub x: f64,
    pub y: f64,
}

/// Flattened: a struct and an internally tagged enum.
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
pub struct Placement {
    pub name: String,
    #[serde(flatten)]
    pub pos: Pos,
    #[serde(flatten)]
    pub extent: Extent,
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
pub struct Args {
    pub body: Body,
    #[serde(default)]
    pub amount: Option<Amount>,
    #[serde(default)]
    pub setting: Option<Setting>,
    #[serde(default)]
    pub placement: Option<Placement>,
}

/// How a command was invoked, like cadatomic's `invocation.rs` (tag `source`).
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum Invocation {
    Palette { args: Args },
    Shortcut { key: String, args: Args },
}

#[svelters::export(module = "command")]
pub fn check_args(args: Json<Args>) -> String {
    format!("{:?}", args.0)
}

#[svelters::export(module = "command")]
pub fn invoke(inv: Json<Invocation>) -> String {
    match inv.0 {
        Invocation::Palette { args } => format!("palette {:?}", args.body),
        Invocation::Shortcut { key, args } => format!("{key} {:?}", args.body),
    }
}

/// A svelters record named like the serde `Amount`: the JSON definition is declared as `AmountJson`.
mod clash {
    #[derive(svelters::Record, Clone, Debug)]
    pub struct Amount {
        pub units: u32,
    }
}

#[svelters::export(module = "command")]
pub fn units(a: clash::Amount, args: Json<Args>) -> u32 {
    a.units + u32::from(args.0.amount.is_some())
}

/// No schema: the path comes from the value alone.
#[svelters::export(module = "command")]
pub fn invoke_any(inv: svelters::JsonAny<Invocation>) -> String {
    invoke(Json(inv.0))
}

// ---- svelters data enums ----

#[derive(svelters::Enum, Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Fast,
    VerySlow,
}

/// A point.
#[derive(svelters::Record, Clone, Debug, PartialEq)]
pub struct Pt {
    pub x: f64,
    pub y: f64,
    pub mode: Mode,
}

/// A shape.
#[derive(svelters::Enum, Clone, Debug, PartialEq)]
#[js(tag = "kind")]
pub enum Shape {
    /// A circle.
    Circle {
        /// The radius.
        r: f64,
        #[js(default = 1)]
        stroke_width: u8,
    },
    Rect {
        w: f64,
        h: f64,
        label: Option<String>,
    },
    #[js(rename = "pt")]
    Point(Pt),
    Empty,
}

/// A layer: a record holding a union list and a serde value.
#[derive(svelters::Record, Clone, Debug)]
pub struct Layer {
    pub name: String,
    pub shapes: Vec<Shape>,
    pub status: Json<SlotStatus>,
}

#[svelters::export(module = "shapes")]
pub fn area(s: Shape) -> f64 {
    match s {
        Shape::Circle { r, .. } => 3.0 * r * r,
        Shape::Rect { w, h, .. } => w * h,
        Shape::Point(_) | Shape::Empty => 0.0,
    }
}

#[svelters::export(module = "shapes")]
pub fn grow(s: Shape, k: f64) -> Shape {
    match s {
        Shape::Circle { r, stroke_width } => Shape::Circle { r: r * k, stroke_width },
        Shape::Rect { w, h, label } => Shape::Rect { w: w * k, h: h * k, label },
        Shape::Point(p) => Shape::Point(Pt { x: p.x * k, y: p.y * k, mode: p.mode }),
        Shape::Empty => Shape::Empty,
    }
}

#[svelters::export(module = "shapes")]
pub fn layer(l: Layer) -> Layer {
    Layer { name: format!("{}!", l.name), shapes: l.shapes.into_iter().rev().collect(), status: l.status }
}

#[svelters::export(module = "shapes")]
pub fn first(shapes: Vec<Shape>) -> Option<Shape> {
    shapes.into_iter().next()
}

// ---- harness ----

struct Run {
    lines: Vec<String>,
    errors: Vec<String>,
}

fn run(name: &str, main: &str) -> Run {
    let dir = std::env::temp_dir().join(format!("svelters-json-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("main.js"), main).unwrap();
    let (cfg, _) = fixture_config();
    let log = LogBackend::new(Vec::new());
    let vt = log.vtable();
    let host = Host::new(vt.backend_ref(), cfg).unwrap();
    if let Err(e) = host.load_bundle(dir.join("main.js")) {
        panic!("load: {e}");
    }
    host.checkpoint();
    host.drain_jobs();
    let lines = host.take_console().into_iter().map(|l| l.text).collect();
    let errors = host.take_errors().into_iter().map(|e| e.to_string()).collect();
    Run { lines, errors }
}

fn has(r: &Run, line: &str) {
    assert!(r.lines.iter().any(|l| l == line), "missing console line {line:?}\nlines: {:#?}\nerrors: {:#?}", r.lines, r.errors);
}

const ERR: &str = "const err = (f) => { try { f(); return 'no error'; } catch (e) { return `${e.name} ${e.domain} ${e.code} ${e.message}`; } };\n";

#[test]
fn json_values_round_trip() {
    let r = run(
        "serde",
        &format!(
            "{ERR}{}",
            r#"
import { echoRefusal, refuse, describeStatus, statuses, sameStatus } from 'svelters:app/intent';
const r = { reason: 'args.missing', message: 'pick a face', data: { missing: ['face'], depth: 2.5 }, severity: 'error', attempts: 2 };
const back = echoRefusal(r);
console.log('echo', back.reason, back.message, back.severity, back.attempts, JSON.stringify(back.data), 'slot' in back);
console.log('refuse', refuse('x').reason, refuse('x').severity, refuse('x').data.missing[0]);
console.log('status', describeStatus({ kind: 'filled', value: 2.5, unit: 'mm' }), '|', describeStatus({ kind: 'empty' }), '|', describeStatus({ kind: 'refused', ...r }));
console.log('statuses', statuses().map((s) => s.kind).join(','), statuses()[1].unit, statuses()[2].reason);
const s = sameStatus({ kind: 'refused', ...r, slot: 'edge' });
console.log('same', s.kind, s.slot, s.attempts, s.data.depth);
console.log('wrong-tag', err(() => describeStatus({ kind: 'hexagon' })));
console.log('no-tag', err(() => describeStatus({ value: 1 })));
console.log('missing', err(() => echoRefusal({ reason: 'r', severity: 'info', attempts: 1 })));
console.log('range', err(() => echoRefusal({ ...r, attempts: 300 })));
console.log('negative', err(() => echoRefusal({ ...r, attempts: -1 })));
console.log('type', err(() => echoRefusal({ ...r, message: 7 })));
console.log('enum', err(() => echoRefusal({ ...r, severity: 'fatal' })));
console.log('inner', err(() => describeStatus({ kind: 'filled', value: 'big' })));
"#
        ),
    );
    assert!(r.errors.is_empty(), "{:#?}", r.errors);
    has(&r, r#"echo args.missing pick a face error 2 {"depth":2.5,"missing":["face"]} false"#);
    has(&r, "refuse x error face");
    has(&r, "status filled 2.5 mm | empty | refused args.missing Error 2");
    has(&r, "statuses empty,filled,refused null args.missing");
    has(&r, "same refused edge 2 2.5");
    has(&r, "wrong-tag NativeError svelters unknown-variant at s.kind: unknown variant `hexagon`, expected one of `empty`, `filled`, `refused`");
    has(&r, "no-tag NativeError svelters missing-field at s: missing field `kind`");
    has(&r, "missing NativeError svelters missing-field at r: missing field `message`");
    has(&r, "range NativeError svelters range at r.attempts: invalid value: integer `300`, expected u8");
    has(&r, "negative NativeError svelters range at r.attempts: invalid value: integer `-1`, expected u8");
    has(&r, "type NativeError svelters type at r.message: invalid type: integer `7`, expected a string");
    has(&r, "enum NativeError svelters unknown-variant at r.severity: unknown variant `fatal`, expected one of `info`, `error`");
    has(&r, r#"inner NativeError svelters type at s.value: invalid type: string "big", expected f64"#);
}

#[test]
fn tagged_enums_round_trip() {
    let r = run(
        "tagged",
        &format!(
            "{ERR}{}",
            r#"
import { area, grow, layer, first } from 'svelters:app/shapes';
console.log('area', area({ kind: 'circle', r: 2 }), area({ kind: 'rect', w: 2, h: 3 }), area({ kind: 'empty' }), area({ kind: 'pt', x: 1, y: 2, mode: 'fast' }));
console.log('grow', JSON.stringify(grow({ kind: 'circle', r: 2, strokeWidth: 3 }, 2)), JSON.stringify(grow({ kind: 'rect', w: 1, h: 2 }, 2)));
console.log('grow-pt', JSON.stringify(grow({ kind: 'pt', x: 1, y: 2, mode: 'very-slow' }, 10)), JSON.stringify(grow({ kind: 'empty' }, 2)));
const l = layer({ name: 'l', shapes: [{ kind: 'circle', r: 1 }, { kind: 'empty' }], status: { kind: 'filled', value: 3, unit: null } });
console.log('layer', l.name, l.shapes.map((s) => s.kind).join(','), JSON.stringify(l.status));
console.log('first', JSON.stringify(first([{ kind: 'rect', w: 1, h: 1, label: 'a' }])), first([]));
console.log('wrong-tag', err(() => area({ kind: 'hexagon' })));
console.log('no-tag', err(() => area({ r: 1 })));
console.log('tag-type', err(() => area({ kind: 1 })));
console.log('missing', err(() => area({ kind: 'rect', w: 2 })));
console.log('range', err(() => area({ kind: 'circle', r: 1, strokeWidth: 300 })));
console.log('type', err(() => area({ kind: 'circle', r: 'big' })));
console.log('unit-enum', err(() => area({ kind: 'pt', x: 1, y: 2, mode: 'warp' })));
console.log('inner-missing', err(() => area({ kind: 'pt', x: 1, mode: 'fast' })));
console.log('in-list', err(() => layer({ name: 'l', shapes: [{ kind: 'empty' }, { kind: 'rect', w: 1 }], status: { kind: 'empty' } })));
console.log('in-json', err(() => layer({ name: 'l', shapes: [], status: { kind: 'filled' } })));
console.log('not-object', err(() => area('circle')));
"#
        ),
    );
    assert!(r.errors.is_empty(), "{:#?}", r.errors);
    has(&r, "area 12 6 0 0");
    has(&r, r#"grow {"kind":"circle","r":4,"strokeWidth":3} {"kind":"rect","w":2,"h":4,"label":null}"#);
    has(&r, r#"grow-pt {"kind":"pt","x":10,"y":20,"mode":"very-slow"} {"kind":"empty"}"#);
    has(&r, r#"layer l! empty,circle {"kind":"filled","unit":null,"value":3}"#);
    has(&r, r#"first {"kind":"rect","w":1,"h":1,"label":"a"} null"#);
    has(&r, "wrong-tag NativeError svelters unknown-variant at s.kind: unknown variant `hexagon`, expected one of `circle`, `rect`, `pt`, `empty`");
    has(&r, "no-tag NativeError svelters missing-field at s: missing field `kind` (the tag of Shape)");
    has(&r, "tag-type NativeError svelters type at s.kind: expected a string tag, got I64(1)");
    has(&r, "missing NativeError svelters missing-field at s: missing field `h`");
    has(&r, "range NativeError svelters range at s.strokeWidth: 300 does not fit u8");
    has(&r, "unit-enum NativeError svelters unknown-variant at s.mode: unknown variant `warp`, expected `fast`, `very-slow`");
    has(&r, "inner-missing NativeError svelters missing-field at s: missing field `y`");
    has(&r, "in-list NativeError svelters missing-field at l.shapes[1]: missing field `h`");
    has(&r, "in-json NativeError svelters missing-field at l.status: missing field `value`");
    let ty = r.lines.iter().find(|l| l.starts_with("type ")).cloned().unwrap_or_default();
    assert!(ty.starts_with("type NativeError svelters type at s.r: "), "{ty}");
    // The host checks the argument is an object before Rust sees it.
    let not_object = r.lines.iter().find(|l| l.starts_with("not-object ")).cloned().unwrap_or_default();
    assert!(not_object.starts_with("not-object TypeError"), "{not_object}");
}

#[test]
fn errors_inside_buffered_serde_types_have_full_paths() {
    let r = run(
        "buffered",
        &format!(
            "{ERR}{}",
            r#"
import { checkArgs, invoke, invokeAny } from 'svelters:app/command';
const body = { kind: 'extrude', extent: { mode: 'blind', depth: 2 }, count: 3 };
const placement = { name: 'p', x: 1, y: 2, mode: 'up_to', face: 'f1' };
console.log('ok', checkArgs({ body, amount: { expr: 'w/2' }, setting: { c: { min: 0, max: 1 }, t: 'range' }, placement }));
console.log('ok-invoke', invoke({ source: 'shortcut', key: 'E', args: { body: { kind: 'hole', r: 1, at: [[0, 1]] } } }));
const extrude = (extent) => ({ body: { kind: 'extrude', extent, count: 1 } });
// Internally tagged inside internally tagged.
console.log('type', err(() => checkArgs(extrude({ mode: 'blind', depth: 'big' }))));
console.log('range', err(() => checkArgs({ body: { ...body, count: 300 } })));
console.log('missing', err(() => checkArgs(extrude({ mode: 'blind' }))));
console.log('tag', err(() => checkArgs(extrude({ mode: 'sideways' }))));
console.log('no-tag', err(() => checkArgs(extrude({ depth: 1 }))));
console.log('optional', err(() => checkArgs(extrude({ mode: 'up_to', face: 'f', offset: 'x' }))));
// `deny_unknown_fields`.
console.log('unknown', err(() => checkArgs({ body: { kind: 'hole', r: 1, at: [], extra: 2 } })));
// Lists inside buffered content.
console.log('list', err(() => checkArgs({ body: { kind: 'hole', r: 1, at: [[0, 0], [1, 'x']] } })));
console.log('length', err(() => checkArgs({ body: { kind: 'hole', r: 1, at: [[0, 0, 0]] } })));
// Untagged.
console.log('untagged', err(() => checkArgs({ body, amount: { expr: 5 } })));
// Adjacently tagged: content after the tag (read directly) and before it (buffered).
console.log('adjacent', err(() => checkArgs({ body, setting: { t: 'range', c: { min: 'a', max: 1 } } })));
console.log('adjacent-buffered', err(() => checkArgs({ body, setting: { c: { min: 'a', max: 1 }, t: 'range' } })));
console.log('adjacent-missing', err(() => checkArgs({ body, setting: { c: { min: 1 }, t: 'range' } })));
console.log('adjacent-tag', err(() => checkArgs({ body, setting: { t: 'slider', c: 1 } })));
// Flattened struct and enum.
console.log('flat', err(() => checkArgs({ body, placement: { ...placement, x: 'big' } })));
console.log('flat-enum', err(() => checkArgs({ body, placement: { ...placement, mode: 'blind' } })));
console.log('flat-type', err(() => checkArgs({ body, placement: { ...placement, mode: 'blind', depth: 'big' } })));
// Three levels of buffering: the invocation, the body, the extent.
console.log('deep', err(() => invoke({ source: 'palette', args: extrude({ mode: 'blind', depth: 'big' }) })));
console.log('deep-unknown', err(() => invoke({ source: 'shortcut', key: 'E', args: { body: { kind: 'hole', r: 1, at: [], extra: 2 } } })));
console.log('deep-untagged', err(() => invoke({ source: 'palette', args: { body, amount: [1] } })));
console.log('deep-adjacent', err(() => invoke({ source: 'palette', args: { body, setting: { t: 'toggle', c: 3 } } })));
// `JsonAny`: no schema, the first value serde would have shown the same way.
console.log('any', err(() => invokeAny({ source: 'palette', args: extrude({ mode: 'blind', depth: 'big' }) })));
console.log('any-missing', err(() => invokeAny({ source: 'palette', args: extrude({ mode: 'blind' }) })));
"#
        ),
    );
    assert!(r.errors.is_empty(), "{:#?}", r.errors);
    has(&r, r#"ok Args { body: Extrude { extent: Blind { depth: 2.0 }, count: 3 }, amount: Some(Expr { expr: "w/2" }), setting: Some(Range { min: 0.0, max: 1.0 }), placement: Some(Placement { name: "p", pos: Pos { x: 1.0, y: 2.0 }, extent: UpTo { face: "f1", offset: None } }) }"#);
    has(&r, "ok-invoke E Hole { r: 1.0, at: [[0.0, 1.0]] }");
    has(&r, r#"type NativeError svelters type at args.body.extent.depth: invalid type: string "big", expected f64"#);
    has(&r, "range NativeError svelters range at args.body.count: invalid value: integer `300`, expected u8");
    has(&r, "missing NativeError svelters missing-field at args.body.extent: missing field `depth`");
    has(&r, "tag NativeError svelters unknown-variant at args.body.extent.mode: unknown variant `sideways`, expected one of `blind`, `through`, `up_to`");
    has(&r, "no-tag NativeError svelters missing-field at args.body.extent: missing field `mode`");
    has(&r, r#"optional NativeError svelters type at args.body.extent.offset: invalid type: string "x", expected f64"#);
    has(&r, "unknown NativeError svelters unknown-field at args.body.extra: unknown field `extra`, expected one of `r`, `at`");
    has(&r, r#"list NativeError svelters type at args.body.at[1][1]: invalid type: string "x", expected f64"#);
    has(&r, "length NativeError svelters type at args.body.at[0]: invalid length 3, expected 2 elements in sequence");
    has(&r, "untagged NativeError svelters invalid at args.amount: data did not match any variant of untagged enum Amount");
    has(&r, r#"adjacent NativeError svelters type at args.setting.c.min: invalid type: string "a", expected f64"#);
    has(&r, r#"adjacent-buffered NativeError svelters type at args.setting.c.min: invalid type: string "a", expected f64"#);
    has(&r, "adjacent-missing NativeError svelters missing-field at args.setting.c: missing field `max`");
    has(&r, "adjacent-tag NativeError svelters unknown-variant at args.setting.t: unknown variant `slider`, expected one of `toggle`, `range`");
    has(&r, r#"flat NativeError svelters type at args.placement.x: invalid type: string "big", expected f64"#);
    has(&r, "flat-enum NativeError svelters missing-field at args.placement: missing field `depth`");
    has(&r, r#"flat-type NativeError svelters type at args.placement.depth: invalid type: string "big", expected f64"#);
    has(&r, r#"deep NativeError svelters type at inv.args.body.extent.depth: invalid type: string "big", expected f64"#);
    has(&r, "deep-unknown NativeError svelters unknown-field at inv.args.body.extra: unknown field `extra`, expected one of `r`, `at`");
    has(&r, "deep-untagged NativeError svelters invalid at inv.args.amount: data did not match any variant of untagged enum Amount");
    has(&r, "deep-adjacent NativeError svelters type at inv.args.setting.c: invalid type: integer `3`, expected a boolean");
    has(&r, r#"any NativeError svelters type at inv.args.body.extent.depth: invalid type: string "big", expected f64"#);
    // Without a schema a missing field cannot be placed: the path stops at the buffered value.
    has(&r, "any-missing NativeError svelters missing-field at inv: missing field `depth`");
}

#[test]
fn typings_and_json_from_one_schema() {
    let s = svelters::exports::schema();
    // SV24: the codegen is a function of the schema value; the free functions use this process's.
    assert_eq!(s.typescript(), svelters::exports::typescript());
    assert_eq!(s.json(), svelters::exports::json());
    let dts = s.typescript();
    for want in [
        "declare module 'svelters:app/shapes' {",
        "export type Shape = { kind: 'circle'; /** The radius. */ r: number; strokeWidth?: number; } | { kind: 'rect'; w: number; h: number; label?: string | null; } | ({ kind: 'pt'; } & Pt) | { kind: 'empty'; };",
        "export interface Pt { x: number; y: number; mode: Mode; }",
        "export type Mode = 'fast' | 'very-slow';",
        "export interface Layer { name: string; shapes: Shape[]; status: SlotStatus; }",
        "export function area(s: Shape): number;",
        "export function first(shapes: Shape[]): Shape | null;",
        "declare module 'svelters:app/intent' {",
        "export function echoRefusal(r: Refusal): Refusal;",
        "export function statuses(): SlotStatus[];",
        "export type Severity = 'info' | 'error';",
    ] {
        assert!(dts.contains(want), "typings lack {want:?}:\n{dts}");
    }
    // The serde types are declared from their JSON Schema, in every module that uses them.
    for module in ["intent", "shapes"] {
        let body = dts.split(&format!("declare module 'svelters:app/{module}' {{")).nth(1).unwrap().split("\ndeclare module").next().unwrap();
        for want in ["export type Refusal = {", "export type SlotStatus = ", "export type Severity = "] {
            assert!(body.contains(want), "{module} lacks {want:?}:\n{body}");
        }
    }
    let refusal = dts.lines().find(|l| l.contains("export type Refusal = ")).unwrap();
    for want in ["/** The reason id (`args.missing`). */ reason: string;", "slot?: string | null;", "message: string;", "data?: unknown;", "severity: Severity;", "attempts: number;"] {
        assert!(refusal.contains(want), "{refusal}");
    }
    let status = dts.lines().find(|l| l.contains("export type SlotStatus = ")).unwrap();
    for want in ["{ kind: 'empty'; }", "{ kind: 'filled'; unit?: string | null; value: number; }", "(Refusal & { kind: 'refused'; })"] {
        assert!(status.contains(want), "{status}");
    }

    let json: serde_json::Value = serde_json::from_str(&s.json()).unwrap();
    let shapes = json["modules"].as_array().unwrap().iter().find(|m| m["name"] == "shapes").unwrap();
    let shape = shapes["types"].as_array().unwrap().iter().find(|t| t["name"] == "Shape").unwrap();
    assert_eq!(shape["kind"], "union");
    assert_eq!(shape["tag"], "kind");
    assert_eq!(shape["variants"][0]["name"], "circle");
    assert_eq!(shape["variants"][0]["docs"], "A circle.");
    assert_eq!(shape["variants"][0]["fields"][1], serde_json::json!({ "name": "strokeWidth", "type": "number", "optional": true }));
    assert_eq!(shape["variants"][2]["inner"], "Pt");
    let intent = json["modules"].as_array().unwrap().iter().find(|m| m["name"] == "intent").unwrap();
    let refusal = intent["types"].as_array().unwrap().iter().find(|t| t["name"] == "Refusal").unwrap();
    assert_eq!(refusal["kind"], "json");
    assert_eq!(refusal["schema"]["properties"]["attempts"]["maximum"], 255);
    assert_eq!(refusal["schema"]["properties"]["severity"]["$ref"], "#/$defs/Severity");

    // SV48: a JSON definition named like a svelters type is declared once, as `<Name>Json`.
    let body = dts.split("declare module 'svelters:app/command' {").nth(1).unwrap().split("\ndeclare module").next().unwrap();
    assert_eq!(body.matches("export interface Amount {").count(), 1, "{body}");
    assert_eq!(body.matches(" Amount = ").count(), 0, "{body}");
    for want in ["export type AmountJson = number | { expr: string; };", "amount?: AmountJson | null;", "export function units(a: Amount, args: Args): number;"] {
        assert!(body.contains(want), "command lacks {want:?}:\n{body}");
    }
    let command = json["modules"].as_array().unwrap().iter().find(|m| m["name"] == "command").unwrap();
    let amount = command["types"].as_array().unwrap().iter().find(|t| t["name"] == "AmountJson").unwrap();
    assert_eq!(amount["kind"], "json");
}
