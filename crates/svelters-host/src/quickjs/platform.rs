//! DX2: the `__svelters_host` ops behind `svelters:clipboard` and `svelters:shell`
//! (`@svelters/native-runtime`'s `src/clipboard.js`, `src/shell.js`). They call the host's
//! [`crate::platform::Platform`] synchronously; the JS modules wrap the results in Promises.
//!
//! - `clipboardWrite(kind, entries)`: `entries` is an array of `[type, string | Uint8Array]`.
//! - `clipboardRead(kind, accept)` -> `[type, string | Uint8Array][]`: the entries whose types
//!   match `accept` (an array of patterns; empty: all). Text types ([`is_text_type`]) that are
//!   UTF-8 come back as strings, everything else as `Uint8Array`s.
//! - `clipboardFormats(kind)` -> `string[]`.
//! - `shellOpen(what, target)`: `what` is `"url"`, `"path"` or `"reveal"`.
//!
//! `kind` is `"clipboard"` or `"primary"`. Failures throw an `Error` with the backend's message.

use super::{core_of, Res};
use crate::core::Core;
use crate::platform::{is_text_type, ClipboardKind, MimeData};
use rquickjs::{Array, Ctx, Exception, Function, Object, TypedArray, Value};
use std::path::Path;
use std::rc::Weak;

fn kind<'js>(ctx: &Ctx<'js>, k: &str) -> Res<ClipboardKind> {
    ClipboardKind::parse(k).ok_or_else(|| Exception::throw_type(ctx, &format!("clipboard: unknown kind {k:?} (\"clipboard\" or \"primary\")")))
}

fn bytes_of<'js>(ctx: &Ctx<'js>, ty: &str, v: &Value<'js>) -> Res<Vec<u8>> {
    if let Some(s) = v.as_string() {
        return Ok(s.to_string()?.into_bytes());
    }
    if let Some(t) = v.as_object().and_then(|o| o.as_typed_array::<u8>()) {
        // SAFETY: the bytes are copied before any JS runs that could detach the buffer.
        return Ok(unsafe { t.as_slice() }.to_vec());
    }
    Err(Exception::throw_type(ctx, &format!("clipboard: the data of {ty:?} must be a string or a Uint8Array")))
}

pub(super) fn install<'js>(ctx: &Ctx<'js>, host: &Object<'js>, core: &Weak<Core>) -> Res<()> {
    let w = core.clone();
    host.set(
        "clipboardWrite",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>, k: String, entries: Array<'js>| -> Res<()> {
            let c = core_of(&ctx, &w)?;
            let k = kind(&ctx, &k)?;
            let mut data = MimeData::new();
            for e in entries.iter::<Array<'js>>() {
                let e = e?;
                let ty: String = e.get(0)?;
                let v: Value<'js> = e.get(1)?;
                let bytes = bytes_of(&ctx, &ty, &v)?;
                data.set(ty, bytes);
            }
            c.cfg.platform.clipboard.write(k, data).map_err(|e| Exception::throw_message(&ctx, &e.to_string()))
        })?,
    )?;
    let w = core.clone();
    host.set(
        "clipboardRead",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>, k: String, accept: Vec<String>| -> Res<Array<'js>> {
            let c = core_of(&ctx, &w)?;
            let k = kind(&ctx, &k)?;
            let accept: Vec<&str> = accept.iter().map(String::as_str).collect();
            let data = c.cfg.platform.clipboard.read(k, &accept).map_err(|e| Exception::throw_message(&ctx, &e.to_string()))?;
            let out = Array::new(ctx.clone())?;
            for (i, (ty, bytes)) in data.into_entries().into_iter().enumerate() {
                let pair = Array::new(ctx.clone())?;
                let bytes = if is_text_type(&ty) {
                    match String::from_utf8(bytes) {
                        Ok(s) => Err(s),
                        Err(e) => Ok(e.into_bytes()),
                    }
                } else {
                    Ok(bytes)
                };
                let v = match bytes {
                    Err(text) => rquickjs::String::from_str(ctx.clone(), &text)?.into_value(),
                    Ok(bytes) => TypedArray::<u8>::new(ctx.clone(), bytes)?.into_value(),
                };
                pair.set(0, ty)?;
                pair.set(1, v)?;
                out.set(i, pair)?;
            }
            Ok(out)
        })?,
    )?;
    let w = core.clone();
    host.set(
        "clipboardFormats",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>, k: String| -> Res<Vec<String>> {
            let c = core_of(&ctx, &w)?;
            let k = kind(&ctx, &k)?;
            c.cfg.platform.clipboard.formats(k).map_err(|e| Exception::throw_message(&ctx, &e.to_string()))
        })?,
    )?;
    let w = core.clone();
    host.set(
        "shellOpen",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>, what: String, target: String| -> Res<()> {
            let c = core_of(&ctx, &w)?;
            let shell = &c.cfg.platform.shell;
            let r = match what.as_str() {
                "url" => shell.open_url(&target),
                "path" => shell.open_path(Path::new(&target)),
                "reveal" => shell.reveal(Path::new(&target)),
                _ => return Err(Exception::throw_type(&ctx, &format!("shell: unknown operation {what:?}"))),
            };
            r.map_err(|e| Exception::throw_message(&ctx, &e.to_string()))
        })?,
    )?;
    Ok(())
}
