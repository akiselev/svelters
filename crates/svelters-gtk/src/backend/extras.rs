//! Native services outside the GIR widget tables: the kind `gio-file`.
//!
//! The generated tables cover properties, signals and the few annotated methods of GTK classes.
//! Methods with callbacks, async completion and `GError` results (the value model's hardest cases)
//! have no entry there yet (dynamic GI invocation is M3), so this module hand-writes one small
//! real-GIO service that uses all of them:
//!
//! | member | kind | what it shows |
//! |---|---|---|
//! | `path` (prop), `uri` (read-only prop) | strings | a mutable native object behind a handle |
//! | `exists()` | sync, bool | |
//! | `loadContents()` | sync, `GError` | `g_file_load_contents`: BYTES out; a failure is `ERROR` (domain `g-io-error-quark`, code `GIOErrorEnum` name) |
//! | `loadContentsEtag()` | out params | the same call with its out parameter: JS gets `{ret: true, contents, etag}`; the backend answers `LIST [true, contents, etag]` |
//! | `loadContentsAsync()` | async | `g_file_load_contents_async` + `GCancellable` (`cancel`): completes through a one-shot `CALLBACK` |
//! | `listNames(fn)` | `call`-scope callback | `g_file_enumerate_children`, invoking the JS function per child, synchronously |
//! | `monitor(fn)` / `unmonitor()` | `notified`-scope callback | `g_file_monitor`: invoked later from the main loop; released when the owner dies |

use super::*;
use gtk::gio::{self, prelude::*};

pub const KIND_FILE: u32 = 0x4001;
const P_PATH: u32 = 0x4101;
const P_URI: u32 = 0x4102;
const M_EXISTS: u32 = 0x4201;
const M_LOAD: u32 = 0x4202;
const M_LOAD_ASYNC: u32 = 0x4203;
const M_LIST: u32 = 0x4204;
const M_MONITOR: u32 = 0x4205;
const M_UNMONITOR: u32 = 0x4206;
const M_LOAD_ETAG: u32 = 0x4207;
pub const T_CB_NAMES: u32 = SVL_T_FIRST_BACKEND + 0xF001;
pub const T_CB_MONITOR: u32 = SVL_T_FIRST_BACKEND + 0xF002;
pub const T_CB_DONE: u32 = SVL_T_FIRST_BACKEND + 0xF003;

pub fn is_extra(kind: u32) -> bool {
    kind >= 0x4000 && kind < 0x5000
}

pub fn kind_by_name(name: &str) -> u32 {
    if name == "gio-file" { KIND_FILE } else { 0 }
}

pub fn find_member(what: u32, kind: u32, name: &str) -> u32 {
    if kind != KIND_FILE {
        return 0;
    }
    match (what, name) {
        (SVL_PROP, "path") => P_PATH,
        (SVL_PROP, "uri") => P_URI,
        (SVL_METHOD, "exists") => M_EXISTS,
        (SVL_METHOD, "loadContents") => M_LOAD,
        (SVL_METHOD, "loadContentsAsync") => M_LOAD_ASYNC,
        (SVL_METHOD, "listNames") => M_LIST,
        (SVL_METHOD, "monitor") => M_MONITOR,
        (SVL_METHOD, "unmonitor") => M_UNMONITOR,
        (SVL_METHOD, "loadContentsEtag") => M_LOAD_ETAG,
        _ => 0,
    }
}

pub fn describe_member(what: u32, member: u32) -> Option<MemberDesc> {
    let s = |name: &str| SlotDesc::new(SVL_STR).named(name);
    let cb = |ty: u32, name: &str| SlotDesc::new(SVL_CALLBACK).typed(ty).named(name);
    let void = SlotDesc::new(SVL_NULL);
    let m = |name: &str, params: Vec<SlotDesc>, ret: SlotDesc, flags: u32| MemberDesc { name: name.into(), params, ret, flags };
    Some(match (what, member) {
        (SVL_PROP, P_PATH) => MemberDesc { name: "path".into(), params: vec![s("path")], ret: s("path"), flags: 0 },
        (SVL_PROP, P_URI) => MemberDesc { name: "uri".into(), params: vec![], ret: s("uri"), flags: 0 },
        (SVL_METHOD, M_EXISTS) => m("exists", vec![], SlotDesc::new(SVL_BOOL), 0),
        (SVL_METHOD, M_LOAD) => m("loadContents", vec![], SlotDesc::new(SVL_BYTES), SVL_M_THROWS),
        (SVL_METHOD, M_LOAD_ETAG) => m(
            "loadContentsEtag",
            vec![SlotDesc::new(SVL_BYTES).named("contents").flags(SVL_D_OUT), s("etag").flags(SVL_D_OUT | SVL_D_NULLABLE)],
            SlotDesc::new(SVL_BOOL),
            SVL_M_THROWS,
        ),
        (SVL_METHOD, M_LOAD_ASYNC) => m("loadContentsAsync", vec![cb(T_CB_DONE, "done")], void, SVL_M_ASYNC | SVL_M_THROWS),
        (SVL_METHOD, M_LIST) => m("listNames", vec![cb(T_CB_NAMES, "visit")], SlotDesc::new(SVL_I64), SVL_M_THROWS),
        (SVL_METHOD, M_MONITOR) => m("monitor", vec![cb(T_CB_MONITOR, "onChange")], void, SVL_M_THROWS),
        (SVL_METHOD, M_UNMONITOR) => m("unmonitor", vec![], SlotDesc::new(SVL_NULL), 0),
        _ => return None,
    })
}

pub fn describe_type(id: u32) -> Option<TypeDesc> {
    let str_ = |n: &str| SlotDesc::new(SVL_STR).named(n);
    Some(match id {
        T_CB_NAMES => TypeDesc {
            name: "(name: string) => boolean".into(),
            kind: SVL_CALLBACK,
            scope: SVL_CB_CALL,
            members: vec![],
            fields: vec![str_("name")],
            ret: SlotDesc::new(SVL_BOOL),
        },
        T_CB_MONITOR => TypeDesc {
            name: "(event: string, path: string) => void".into(),
            kind: SVL_CALLBACK,
            scope: SVL_CB_NOTIFIED,
            members: vec![],
            fields: vec![str_("event"), str_("path")],
            ret: SlotDesc::new(SVL_NULL),
        },
        T_CB_DONE => TypeDesc {
            name: "(result: Uint8Array | NativeError) => void".into(),
            kind: SVL_CALLBACK,
            scope: SVL_CB_ASYNC,
            members: vec![],
            fields: vec![SlotDesc::new(SVL_NULL).named("result")],
            ret: SlotDesc::new(SVL_NULL),
        },
        _ => return None,
    })
}

#[derive(Default)]
pub(super) struct FileState {
    path: String,
    /// Running async operations by completion callback id.
    cancels: HashMap<u64, gio::Cancellable>,
    /// The monitor and the callback id its changes go to.
    monitor: Option<(gio::FileMonitor, glib::SignalHandlerId, u64)>,
}

/// A failure of a call: a plain message, or a `GError` that crosses as an `ERROR` value.
pub(super) enum CallErr {
    Plain(String),
    Gerror(glib::Error),
}

impl From<String> for CallErr {
    fn from(s: String) -> CallErr {
        CallErr::Plain(s)
    }
}

impl From<&str> for CallErr {
    fn from(s: &str) -> CallErr {
        CallErr::Plain(s.to_string())
    }
}

impl From<glib::Error> for CallErr {
    fn from(e: glib::Error) -> CallErr {
        CallErr::Gerror(e)
    }
}

pub(super) fn args_of<'a>(args: *const svl_value, n: usize) -> &'a [svl_value] {
    if args.is_null() || n == 0 { &[] } else { unsafe { std::slice::from_raw_parts(args, n) } }
}

fn callback_id(v: &svl_value) -> Result<u64, String> {
    if v.tag != SVL_CALLBACK {
        return Err("expected a callback".into());
    }
    Ok(unsafe { v.u.u })
}

impl Inner {
    pub(super) fn extra_kind(&self, h: u64) -> Option<u32> {
        self.state.borrow().entries.get(&h).map(|e| e.kind).filter(|k| is_extra(*k) && !super::menus::is_menu_kind(*k))
    }

    pub(super) fn create_extra(&self, kind: u32) -> Res<u64> {
        if kind != KIND_FILE {
            return Err("unknown kind".into());
        }
        let obj = glib::Object::new::<glib::Object>();
        let mut st = self.state.borrow_mut();
        st.next += 1;
        let h = st.next;
        st.entries.insert(
            h,
            Entry { obj: obj.downgrade(), hold: Some(obj.clone()), class: SVL_LIFE_OWNED, owner: 0, note: std::ptr::null_mut(), kind, signals: HashMap::new(), slot: None, capture_prev: None, observers: Default::default() },
        );
        drop(st);
        self.tag_object(&obj, h);
        self.files.borrow_mut().insert(h, FileState::default());
        Ok(h)
    }

    /// Releases what a dying `gio-file` holds: running operations are cancelled, the monitor is
    /// stopped, and the callback it kept is handed back to the host.
    pub(super) fn destroy_extra(&self, h: u64) {
        let Some(st) = self.files.borrow_mut().remove(&h) else { return };
        for c in st.cancels.values() {
            c.cancel();
        }
        if let Some((mon, id, cb)) = st.monitor {
            mon.disconnect(id);
            mon.cancel();
            self.host_release(cb);
        }
    }

    pub(super) fn extra_set_prop(&self, h: u64, prop: u32, v: &svl_value) -> Res<()> {
        match prop {
            P_PATH => {
                let s = unsafe { convert::as_str(v) }.ok_or("expected string")?.into_owned();
                self.files.borrow_mut().get_mut(&h).ok_or("stale handle")?.path = s;
                Ok(())
            }
            P_URI => Err("property uri is read-only".into()),
            _ => Err("unknown property".into()),
        }
    }

    pub(super) fn extra_get_prop(&self, h: u64, prop: u32, out: &mut svl_value) -> Res<()> {
        let path = self.files.borrow().get(&h).ok_or("stale handle")?.path.clone();
        let mut buf = self.out.borrow_mut();
        *buf = StrBuf::default();
        *out = match prop {
            P_PATH => buf.str_value(&path),
            P_URI => buf.str_value(gio::File::for_path(&path).uri().as_str()),
            _ => return Err("unknown property".into()),
        };
        Ok(())
    }

    fn file_of(&self, h: u64) -> Result<gio::File, String> {
        let files = self.files.borrow();
        let st = files.get(&h).ok_or("stale handle")?;
        if st.path.is_empty() {
            return Err("set `path` first".into());
        }
        Ok(gio::File::for_path(&st.path))
    }

    /// The code of a `GError` as the ABI carries it: a `GIOErrorEnum` member for the GIO domain.
    fn gerror_value(&self, e: &glib::Error) -> Value {
        let domain = e.domain().as_str().to_string();
        let code = if domain == "g-io-error-quark" {
            let ty = self.types.borrow_mut().id(gio::IOErrorEnum::static_type());
            let value = unsafe { glib::translate::IntoGlib::into_glib(e.kind::<gio::IOErrorEnum>().unwrap_or(gio::IOErrorEnum::Failed)) };
            Value::Enum { ty, value: value as i64 }
        } else {
            Value::I64(0)
        };
        Value::Error(Box::new(NativeError { domain, code, message: e.message().to_string() }))
    }

    pub(super) fn write_error(&self, e: &glib::Error, out: &mut svl_value) {
        let Value::Error(n) = self.gerror_value(e) else { return };
        let mut buf = self.out.borrow_mut();
        *buf = StrBuf::default();
        let dom = buf.str_value(&n.domain);
        let code = match &n.code {
            Value::Enum { ty, value } => svl_value::typed(SVL_ENUM, *ty, svl_value_u { i: *value }),
            _ => svl_value::new(SVL_I64, svl_value_u { i: 0 }),
        };
        let msg = buf.str_value(&n.message);
        *out = buf.list_value(SVL_ERROR, 0, vec![dom, code, msg]);
    }

    /// Calls the JS function behind a callback id. `Err` carries the host's reason (released, owner gone, threw).
    fn host_invoke(&self, id: u64, args: &[Value]) -> Result<Value, String> {
        let host = self.host.load(Ordering::Acquire);
        if host.is_null() {
            return Err("no host".into());
        }
        unsafe { HostRef::from_raw(host) }.invoke_callback(id, args).map_err(|e| e.message)
    }

    fn host_release(&self, id: u64) {
        let host = self.host.load(Ordering::Acquire);
        if !host.is_null() {
            unsafe { HostRef::from_raw(host) }.release_callback(id);
        }
    }

    pub(super) fn extra_call(self: &Rc<Self>, h: u64, method: u32, args: *const svl_value, n: usize, out: &mut svl_value) -> Result<(), CallErr> {
        let args = args_of(args, n);
        *out = convert::null_value();
        match method {
            M_EXISTS => {
                let f = self.file_of(h)?;
                *out = svl_value::new(SVL_BOOL, svl_value_u { b: f.query_exists(None::<&gio::Cancellable>) });
            }
            M_LOAD => {
                let f = self.file_of(h)?;
                let (bytes, _etag) = f.load_contents(None::<&gio::Cancellable>)?;
                let mut buf = self.out.borrow_mut();
                *buf = StrBuf::default();
                *out = buf.bytes_value(bytes.to_vec());
            }
            M_LOAD_ETAG => {
                let f = self.file_of(h)?;
                let (bytes, etag) = f.load_contents(None::<&gio::Cancellable>)?;
                let mut buf = self.out.borrow_mut();
                *buf = StrBuf::default();
                let ok = svl_value::new(SVL_BOOL, svl_value_u { b: true });
                let contents = buf.bytes_value(bytes.to_vec());
                let etag = match etag {
                    Some(t) => buf.str_value(t.as_str()),
                    None => convert::null_value(),
                };
                *out = buf.list_value(SVL_LIST, 0, vec![ok, contents, etag]);
            }
            M_LOAD_ASYNC => {
                let f = self.file_of(h)?;
                let id = callback_id(args.last().ok_or("missing completion callback")?)?;
                let cancellable = gio::Cancellable::new();
                self.files.borrow_mut().get_mut(&h).ok_or("stale handle")?.cancels.insert(id, cancellable.clone());
                let weak = Rc::downgrade(self);
                f.load_contents_async(Some(&cancellable), move |res| {
                    let Some(inner) = weak.upgrade() else { return };
                    if let Some(st) = inner.files.borrow_mut().get_mut(&h) {
                        st.cancels.remove(&id);
                    }
                    let v = match res {
                        Ok((bytes, _etag)) => Value::Bytes(bytes.to_vec()),
                        Err(e) => inner.gerror_value(&e),
                    };
                    // A completion for a callback that is gone (owner destroyed) is dropped by the host.
                    let _ = inner.host_invoke(id, &[v]);
                });
            }
            M_LIST => {
                let f = self.file_of(h)?;
                let id = callback_id(args.first().ok_or("missing callback")?)?;
                let it = f.enumerate_children("standard::name", gio::FileQueryInfoFlags::NONE, None::<&gio::Cancellable>)?;
                let mut count = 0i64;
                for info in it {
                    let info = info?;
                    count += 1;
                    let keep_going = self.host_invoke(id, &[Value::Str(info.name().to_string_lossy().into_owned())])?;
                    if keep_going == Value::Bool(false) {
                        break;
                    }
                }
                *out = svl_value::new(SVL_I64, svl_value_u { i: count });
            }
            M_MONITOR => {
                let f = self.file_of(h)?;
                let id = callback_id(args.first().ok_or("missing callback")?)?;
                if self.files.borrow().get(&h).is_some_and(|s| s.monitor.is_some()) {
                    return Err("already monitoring".into());
                }
                let mon = f.monitor(gio::FileMonitorFlags::NONE, None::<&gio::Cancellable>)?;
                let weak = Rc::downgrade(self);
                let handler = mon.connect_changed(move |_, file, _other, event| {
                    let Some(inner) = weak.upgrade() else { return };
                    let nick = glib::EnumClass::with_type(gio::FileMonitorEvent::static_type())
                        .and_then(|c| c.value(unsafe { glib::translate::IntoGlib::into_glib(event) }).map(|v| v.nick().to_string()))
                        .unwrap_or_default();
                    let path = file.path().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
                    let _ = inner.host_invoke(id, &[Value::Str(nick), Value::Str(path)]);
                });
                self.files.borrow_mut().get_mut(&h).ok_or("stale handle")?.monitor = Some((mon, handler, id));
            }
            M_UNMONITOR => {
                let taken = self.files.borrow_mut().get_mut(&h).and_then(|s| s.monitor.take());
                if let Some((mon, id, cb)) = taken {
                    mon.disconnect(id);
                    mon.cancel();
                    self.host_release(cb);
                }
            }
            _ => return Err("unknown method".into()),
        }
        Ok(())
    }

    /// `backend->cancel(callback)`: cancel the async operation completing `callback`.
    pub(super) fn cancel_callback(&self, callback: u64) -> bool {
        for st in self.files.borrow().values() {
            if let Some(c) = st.cancels.get(&callback) {
                c.cancel();
                return true;
            }
        }
        false
    }
}
