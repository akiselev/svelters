//! `setProp` micro-benchmark on the real host path (QuickJS -> host core -> backend vtable), against
//! a dummy backend, as in `docs/notes/js-native-serdes.md` section 4.
//!
//!   CARGO_BUILD_JOBS=4 cargo run --release -p svelters-host --example setprop_bench
//!
//! Rows: the JS loop alone, then `setProp` by name and by id for an int, an ASCII string, a Latin-1
//! string, a wide string, and a record. Every figure is the best of 7 rounds, in ns per op, JS loop
//! included. The same JS runs against a backend that takes a borrowed `ValueRef` (the new trait
//! method) and against one that takes an owned `Value` (the only option before).

use std::hint::black_box;
use std::sync::Arc;
use svelters_abi::*;
use svelters_host::{Host, HostConfig};

struct Sink {
    utf16: bool,
    borrowed: bool,
}

const PROPS: &[&str] = &["spacing", "label", "geometry"];

impl Backend for Sink {
    fn resolve(&self, what: NameKind, kind: svl_id, name: &str) -> svl_id {
        match what {
            NameKind::Kind => 1,
            NameKind::Prop if kind == 1 => PROPS.iter().position(|p| *p == name).map_or(0, |i| i as u32 + 1),
            _ => 0,
        }
    }
    fn create(&self, _: svl_id) -> StrResult<svl_handle> {
        Ok(1)
    }
    fn insert(&self, _: svl_handle, _: svl_handle, _: Option<svl_handle>, _: svl_id) -> StrResult<()> {
        Ok(())
    }
    fn remove(&self, _: svl_handle, _: svl_handle) -> StrResult<()> {
        Ok(())
    }
    fn destroy(&self, _: svl_handle) {}
    fn set_prop(&self, _: svl_handle, _: svl_id, v: &Value) -> StrResult<()> {
        black_box(v);
        Ok(())
    }
    fn set_prop_ref(&self, h: svl_handle, p: svl_id, v: ValueRef<'_>) -> StrResult<()> {
        if self.borrowed {
            black_box((v.tag(), v.flags(), v.raw().type_));
            if v.tag() == SVL_STR {
                // Like the Qt backend: read the code units where they are (`QString::fromUtf16` is
                // a memcpy), no transcoding.
                let s = unsafe { v.raw().u.s };
                black_box((s.len, unsafe { *s.ptr.cast::<u8>() }));
            }
            Ok(())
        } else {
            self.set_prop(h, p, &v.to_value())
        }
    }
    fn get_prop(&self, _: svl_handle, _: svl_id) -> StrResult<Value> {
        Ok(Value::Null)
    }
    fn call(&self, _: svl_handle, _: svl_id, _: &[Value]) -> StrResult<Value> {
        Ok(Value::Null)
    }
    fn listen(&self, _: svl_handle, _: svl_id, _: bool) -> StrResult<()> {
        Ok(())
    }
    fn run(&self, _: &HostRef<'_>) -> StrResult<()> {
        Ok(())
    }
    fn waker(&self) -> Waker {
        Arc::new(|| {})
    }
    fn quit(&self) {}
    fn schedule_wake(&self, _: i64) {}
    fn handle_info(&self, _: svl_handle) -> StrResult<svl_handle_info> {
        Ok(svl_handle_info { kind: 1, lifetime: 1, flags: 0 })
    }
    fn caps(&self) -> u32 {
        if self.utf16 { SVL_CAP_UTF16_IN } else { 0 }
    }
    fn describe(&self, what: NameKind, _: svl_id, member: svl_id) -> Option<MemberDesc> {
        if what != NameKind::Prop {
            return None;
        }
        let slot = match member {
            1 => SlotDesc::new(SVL_I64).range(-1.0, 1000.0),
            2 => SlotDesc::new(SVL_STR).flags(SVL_D_NULLABLE),
            3 => SlotDesc::new(SVL_RECORD).typed(SVL_T_FIRST_BACKEND + 1),
            _ => return None,
        };
        let name = PROPS[member as usize - 1].to_string();
        let slot = slot.named(&name);
        Some(MemberDesc { name, params: vec![slot.clone()], ret: slot, flags: 0 })
    }
    fn describe_type(&self, ty: svl_id) -> Option<TypeDesc> {
        (ty == SVL_T_FIRST_BACKEND + 1).then(|| TypeDesc {
            name: "Rect".into(),
            kind: SVL_RECORD, scope: 0,
            members: vec![],
            fields: ["x", "y", "width", "height"].iter().map(|n| SlotDesc::new(SVL_F64).named(n)).collect(),
            ret: SlotDesc::new(SVL_NULL),
        })
    }
}

const JS: &str = r#"
const h = globalThis.__svelters_host;
const t = h.create('thing');
const byId = typeof h.propId === 'function';
const kid = byId ? h.kindId('thing') : 0;
const pSpacing = byId ? h.propId(kid, 'spacing') : 'spacing';
const pLabel = byId ? h.propId(kid, 'label') : 'label';
const pGeom = byId ? h.propId(kid, 'geometry') : 'geometry';
const ascii = 'hello world';
const kib = 'x'.repeat(1024);
const latin1 = 'é'.repeat(1024);
const wide = '✓'.repeat(1024);
const rect = { x: 1, y: 2, width: 300, height: 400 };
function bench(label, f, n = 20000) {
  f(200);
  let best = 1e18;
  for (let r = 0; r < 7; r++) {
    const t0 = performance.now();
    f(n);
    best = Math.min(best, ((performance.now() - t0) * 1e6) / n);
  }
  console.log(label.padEnd(44) + String(Math.round(best)).padStart(7) + ' ns/op');
}
bench('JS loop only', (n) => { for (let i = 0; i < n; i++); });
bench('setProp(h, "spacing", int)  by name', (n) => { for (let i = 0; i < n; i++) h.setProp(t, 'spacing', i & 255); });
if (byId) bench('setProp(h, id, int)         by id', (n) => { for (let i = 0; i < n; i++) h.setProp(t, pSpacing, i & 255); });
bench('setProp label 11 B ASCII      by name', (n) => { for (let i = 0; i < n; i++) h.setProp(t, 'label', ascii); });
if (byId) bench('setProp label 11 B ASCII      by id', (n) => { for (let i = 0; i < n; i++) h.setProp(t, pLabel, ascii); });
bench('setProp label 1 KiB ASCII     by id/name', (n) => { for (let i = 0; i < n; i++) h.setProp(t, pLabel, kib); });
bench('setProp label 1 KiB Latin-1   by id/name', (n) => { for (let i = 0; i < n; i++) h.setProp(t, pLabel, latin1); }, 5000);
bench('setProp label 1 KiB wide      by id/name', (n) => { for (let i = 0; i < n; i++) h.setProp(t, pLabel, wide); }, 5000);
try { h.setProp(t, pGeom, rect); bench('setProp geometry {x,y,width,height}   ', (n) => { for (let i = 0; i < n; i++) h.setProp(t, pGeom, rect); }, 10000); } catch (e) { console.log('geometry: ' + e.message); }
"#;

fn run(label: &str, backend: Sink) {
    let vt = BackendVtable::new(backend);
    let host = Host::new(vt.backend_ref(), HostConfig { echo_console: false, echo_errors: false, turn_budget: None, ..HostConfig::default() }).unwrap();
    let path = std::env::temp_dir().join("svelters_setprop_bench.js");
    std::fs::write(&path, JS).unwrap();
    println!("== {label}");
    host.load_bundle(&path).unwrap();
    for l in host.take_console() {
        println!("{}", l.text);
    }
    for e in host.take_errors() {
        println!("error: {e}");
    }
}

fn main() {
    run("backend takes a borrowed ValueRef, UTF-8 only (GTK-like)", Sink { utf16: false, borrowed: true });
    run("backend takes a borrowed ValueRef, accepts UTF-16 (Qt-like)", Sink { utf16: true, borrowed: true });
    run("backend takes an owned Value (the only option before)", Sink { utf16: false, borrowed: false });
}
