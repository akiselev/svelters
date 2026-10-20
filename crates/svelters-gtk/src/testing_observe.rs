//! Verification of observe facts (milestone R0; plans/research/REACTIVITY.md section 4.5).
//!
//! `tools/metadata/gtk/gen_backend.py` turns every observe fact of `metadata/observe/gtk.json` into one
//! [`Row`] of a table test per owner type (`generated::verify::observe_<ns>_<type>`), so a thousand facts are a
//! few hundred test processes, not a thousand. [`table`] runs the rows through the real ABI path
//! (`observe` -> `notify::<prop>` -> `host.prop_changed`), each on a fresh harness, and reports every row:
//! with `SVELTERS_TEST_RESULTS=<file>` it appends one JSON line per row start and result
//! (`{"test": "<table>#<prop>", "result": "start"|"pass"|"fail", "message"}`), which
//! `tools/metadata/ledger.py refresh` records per fact. The table test fails when any row fails.
//!
//! Checks (the `verify.check` of the fact):
//! - `Change`: the property is set natively (GObject, not the ABI) to a new value derived from its
//!   `GParamSpec` (same rule as the probe, tools/metadata/gtk/observe_probe.c): announced at least once;
//!   an identical set is (on_set) or is not (on_change) announced again; after `observe(off)` nothing is;
//!   destroying the handle while observed ends the observation.
//! - `Cross`: another property is set, or a statement runs; this property changes and is announced.
//! - `Via`: the forwarded-to object's property is set natively; the element's property is announced.
//! - `Silent`: a change the fact says is missed (class none, or a partial fact's missed cause): the
//!   property changes and nothing is announced.
//! - `Connect`: the mechanism installs and uninstalls (no native trigger known; `verified: connect`).
//! - `Refused`: `observe` answers -1 (constant).
//!
//! Facts are made for the GTK version of the ingested metadata. On an older GTK (the 4.14 CI job) a row
//! only checks that the mechanism installs and uninstalls, and a type or property the running GTK lacks
//! skips the row.

use super::Harness;
use gtk::glib;
use gtk::glib::translate::ToGlibPtr;
use gtk::prelude::*;
use std::io::Write;

/// How the row's instance is made: an element by tag (generated ids) or a type by GIR name (`construct`).
pub enum On {
    Create(&'static str),
    Construct(&'static str),
}

/// A native change that is not a set of the observed property itself.
pub enum Trig {
    /// Set this other property of the same instance to a new value.
    Set(&'static str),
    /// Run a statement on the instance (from `metadata/rules/gtk.yaml` `observe.triggers`).
    Rust(fn(&glib::Object)),
}

pub enum Check {
    Change,
    Cross(Trig),
    /// (the property holding the forwarded-to object, the property on it)
    Via(&'static str, &'static str),
    Silent(Trig),
    Connect,
    Refused,
}

#[derive(PartialEq)]
pub enum Fires {
    OnChange,
    OnSet,
    Any,
}

pub struct Row {
    pub prop: &'static str,
    pub on: On,
    pub check: Check,
    pub fires: Fires,
    /// GTK version the fact needs (kind or property `since`); older: the row skips.
    pub since: (u16, u16),
    /// Statement run on the new instance first (`observe.setup` in the rules).
    pub setup: Option<fn(&glib::Object)>,
}

/// The GTK version the observe facts were made for (the ingested metadata); older GTK runs install checks only.
pub const FACTS_GTK: (u16, u16) = (4, 22);

/// A different value within the pspec's range, or None (unsupported type, or no room).
pub fn next_value(ps: &glib::ParamSpec, cur: &glib::Value) -> Option<glib::Value> {
    macro_rules! int {
        ($spec:ty, $t:ty) => {
            if let Some(s) = ps.downcast_ref::<$spec>() {
                let c: $t = cur.get().ok()?;
                let v = if c < s.maximum() { c + 1 } else { c - 1 };
                return (v >= s.minimum() && v <= s.maximum() && v != c).then(|| v.to_value());
            }
        };
    }
    if let Some(_) = ps.downcast_ref::<glib::ParamSpecBoolean>() {
        return Some((!cur.get::<bool>().ok()?).to_value());
    }
    int!(glib::ParamSpecInt, i32);
    int!(glib::ParamSpecUInt, u32);
    int!(glib::ParamSpecInt64, i64);
    int!(glib::ParamSpecUInt64, u64);
    int!(glib::ParamSpecLong, glib::ffi::glong);
    int!(glib::ParamSpecULong, glib::ffi::gulong);
    if ps.downcast_ref::<glib::ParamSpecUnichar>().is_some() {
        // A gunichar property holds a G_TYPE_UINT value: the next valid code point.
        let c: u32 = unsafe { glib::gobject_ffi::g_value_get_uint(cur.to_glib_none().0) };
        let v = char::from_u32(c + 1).unwrap_or('A');
        return Some((v as u32).to_value());
    }
    // The first candidate inside the range that differs from the current value: c + 1 and c - 0.5 vanish next to
    // a huge default (GtkRange:fill-level is G_MAXDOUBLE), so 0, 1 and the middle of the range follow.
    if let Some(s) = ps.downcast_ref::<glib::ParamSpecDouble>() {
        let c: f64 = cur.get().ok()?;
        let (lo, hi) = (s.minimum(), s.maximum());
        return [c + 1.0, c - 0.5, 0.0, 1.0, lo / 2.0 + hi / 2.0].into_iter().find(|v| *v >= lo && *v <= hi && *v != c).map(|v| v.to_value());
    }
    if let Some(s) = ps.downcast_ref::<glib::ParamSpecFloat>() {
        let c: f32 = cur.get().ok()?;
        let (lo, hi) = (s.minimum(), s.maximum());
        return [c + 1.0, c - 0.5, 0.0, 1.0, lo / 2.0 + hi / 2.0].into_iter().find(|v| *v >= lo && *v <= hi && *v != c).map(|v| v.to_value());
    }
    if ps.downcast_ref::<glib::ParamSpecString>().is_some() {
        let c: Option<String> = cur.get().ok()?;
        return Some(format!("{}x", c.unwrap_or_default()).to_value());
    }
    if ps.downcast_ref::<glib::ParamSpecEnum>().is_some() {
        let ec = glib::EnumClass::with_type(ps.value_type())?;
        let c = unsafe { glib::gobject_ffi::g_value_get_enum(cur.to_glib_none().0) };
        let v = ec.values().iter().map(|x| x.value()).find(|v| *v != c)?;
        return ec.to_value(v);
    }
    None
}

/// Sets `name` of `obj` natively to a new value; Err when the type has no next value.
fn set_next(obj: &glib::Object, name: &str) -> Result<(), String> {
    let ps = obj.find_property(name).ok_or_else(|| format!("{} has no property {name}", obj.type_().name()))?;
    let cur = obj.property_value(name);
    let v = next_value(&ps, &cur).ok_or_else(|| format!("no new value for {name} ({})", ps.value_type().name()))?;
    obj.set_property_from_value(name, &v);
    Ok(())
}

fn fire(obj: &glib::Object, t: &Trig) -> Result<(), String> {
    match t {
        Trig::Set(p) => set_next(obj, p),
        Trig::Rust(f) => {
            f(obj);
            Ok(())
        }
    }
}

/// A snapshot of the observed property as the host would read it (shape only: boxed reads mint handles).
fn snap(h: &Harness, o: u64, prop: &str) -> String {
    super::shape(&h.get_value(o, prop))
}

enum Outcome {
    Pass,
    Skip(String),
}

fn run(r: &Row) -> Result<Outcome, String> {
    if !crate::compat::has(r.since) {
        return Ok(Outcome::Skip(format!("needs GTK {}.{}", r.since.0, r.since.1)));
    }
    let full = crate::compat::has(FACTS_GTK);
    let h = Harness::new();
    let (kind, o) = match r.on {
        On::Create(tag) => {
            if h.try_kind_id(tag).is_none() {
                return Ok(Outcome::Skip(format!("no kind {tag} on this GTK")));
            }
            (tag, h.try_create(tag)?)
        }
        On::Construct(name) => {
            if h.try_kind_id(name).is_none() {
                return Ok(Outcome::Skip(format!("no type {name} on this GTK")));
            }
            (name, h.construct(name, &[])?)
        }
    };
    let obj = h.obj(o);
    if let Some(f) = r.setup {
        f(&obj);
    }
    let pid = h.resolve_prop(kind, r.prop);
    if pid == 0 {
        return Ok(Outcome::Skip(format!("{kind} has no property {} on this GTK", r.prop)));
    }
    // An older GTK can lack the GObject property (GtkIconPaintable `scale`/`size`) or have it construct-only
    // (GtkIconPaintable `is-symbolic`, GtkMapListModel `model` on 4.14) where the facts' GTK lets it change.
    if !full && !matches!(r.check, Check::Refused | Check::Via(..)) {
        match obj.find_property(r.prop) {
            None => return Ok(Outcome::Skip(format!("{kind} has no GObject property {} on this GTK", r.prop))),
            Some(ps) if ps.flags().contains(glib::ParamFlags::CONSTRUCT_ONLY) => {
                return Ok(Outcome::Skip(format!("{} is construct-only on this GTK", r.prop)));
            }
            Some(_) => {}
        }
    }
    let count = |h: &Harness| h.take_changed().into_iter().filter(|c| *c == (o, pid)).count();
    if let Check::Refused = r.check {
        return match h.observe_id(o, pid, true) {
            Err((-1, _)) => Ok(Outcome::Pass),
            Err((rc, m)) => Err(format!("observe refused with {rc} ({m}), want -1")),
            Ok(()) => Err("observe accepted a constant property".into()),
        };
    }
    h.observe_id(o, pid, true).map_err(|(rc, m)| format!("observe failed ({rc}): {m}"))?;
    if !full || matches!(r.check, Check::Connect) {
        h.observe_id(o, pid, false).map_err(|(rc, m)| format!("observe(off) failed ({rc}): {m}"))?;
        return Ok(Outcome::Pass);
    }
    // The object the native change is made on, and how.
    let change = |h: &Harness| -> Result<(), String> {
        match &r.check {
            Check::Change => set_next(&obj, r.prop),
            Check::Cross(t) | Check::Silent(t) => fire(&obj, t),
            Check::Via(holder, target) => {
                let tgt = obj.property::<Option<glib::Object>>(holder).ok_or_else(|| format!("{holder} is unset"))?;
                set_next(&tgt, target)
            }
            Check::Connect | Check::Refused => unreachable!(),
        }?;
        let _ = h;
        Ok(())
    };
    h.take_changed();
    let before = snap(&h, o, r.prop);
    change(&h)?;
    let n = count(&h);
    let after = snap(&h, o, r.prop);
    if before == after && !matches!(r.check, Check::Silent(_)) {
        // Shapes hide handle values (object props): only a scalar that did not move is a broken precondition.
        if !after.contains('#') {
            return Err(format!("the native change left {} at {after}", r.prop));
        }
    }
    if let Check::Silent(_) = r.check {
        if before == after {
            return Err(format!("the trigger did not change {} ({after})", r.prop));
        }
        if n != 0 {
            return Err(format!("{} was announced {n} time(s); the fact says this change is missed", r.prop));
        }
        h.observe_id(o, pid, false).map_err(|(rc, m)| format!("observe(off) failed ({rc}): {m}"))?;
        return Ok(Outcome::Pass);
    }
    if n == 0 {
        return Err(format!("a native change of {} was not announced", r.prop));
    }
    if let Check::Change = r.check {
        // An identical set: GObject re-notifies unless the property is EXPLICIT_NOTIFY and checks.
        let cur = obj.property_value(r.prop);
        obj.set_property_from_value(r.prop, &cur);
        let same = count(&h);
        match r.fires {
            Fires::OnChange if same != 0 => return Err(format!("an identical set was announced {same} time(s) (fires: on_set, not on_change)")),
            Fires::OnSet if same == 0 => return Err("an identical set was not announced (fires: on_change, not on_set)".into()),
            _ => {}
        }
    }
    // Unobserved: silent.
    h.observe_id(o, pid, false).map_err(|(rc, m)| format!("observe(off) failed ({rc}): {m}"))?;
    change(&h)?;
    let n = count(&h);
    if n != 0 {
        return Err(format!("{n} announcement(s) after observe(off)"));
    }
    // Destroyed while observed: the observation ends with the handle (our own reference keeps the object).
    h.observe_id(o, pid, true).map_err(|(rc, m)| format!("observe failed ({rc}): {m}"))?;
    h.destroy(o);
    let _ = change(&h);
    let n = count(&h);
    if n != 0 {
        return Err(format!("{n} announcement(s) after the handle was destroyed"));
    }
    Ok(Outcome::Pass)
}

fn report(line: &serde_json::Value) {
    if let Some(path) = std::env::var_os("SVELTERS_TEST_RESULTS") {
        // One write per line on an O_APPEND file: tables running in parallel processes do not interleave.
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = f.write_all(format!("{line}\n").as_bytes());
        }
    }
}

/// Runs a table of observe rows on the GTK thread; panics listing the failed rows.
pub fn table(name: &'static str, rows: Vec<Row>) {
    super::on_gtk(move || {
        let mut failed = Vec::new();
        for r in &rows {
            let test = format!("{name}#{}", r.prop);
            report(&serde_json::json!({"test": test, "result": "start"}));
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(r)))
                .unwrap_or_else(|p| Err(p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_else(|| "panicked".into())));
            match res {
                Ok(Outcome::Pass) => report(&serde_json::json!({"test": test, "result": "pass"})),
                Ok(Outcome::Skip(why)) => {
                    eprintln!("SKIP {test}: {why}");
                    report(&serde_json::json!({"test": test, "result": "pass", "message": format!("skipped: {why}")}));
                }
                Err(m) => {
                    eprintln!("FAIL {test}: {m}");
                    report(&serde_json::json!({"test": test, "result": "fail", "message": m}));
                    failed.push(format!("{}: {m}", r.prop));
                }
            }
        }
        assert!(failed.is_empty(), "{} observe row(s) failed:\n  {}", failed.len(), failed.join("\n  "));
    });
}
