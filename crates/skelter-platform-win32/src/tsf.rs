//! The TSF text store: an `ITextStoreACP` (plus `ITfContextOwnerCompositionSink`) over the focused
//! field's [`TextInputClient`], following the designs of Chromium's `TSFTextStore` and Firefox's
//! `TSFTextStore` (`plans/research/skelter/notes/round2/ime-a11y-os.md` §1.4).
//!
//! - **Offsets** are UTF-16 code units of the whole document, which is what the client speaks too.
//! - **Locks** follow the documented protocol: TSF asks with `RequestLock`, the store grants it by
//!   calling `OnLockGranted` before returning (synchronous) or later (queued, `TS_S_ASYNC`). The
//!   client lives inside the app's handler, so while the handler runs (an OS call made from inside
//!   it re-entered TSF) synchronous requests are refused with `TS_E_SYNCHRONOUS` and asynchronous
//!   ones wait for [`TsfDocument::grant_pending`] after the handler returns ([`LockState`]).
//! - **Edits** from the TIP go straight to the client: `replace_text_in_range`, or
//!   `replace_and_mark_text_in_range` during a composition. When the lock ends, the client's marked
//!   range is set to the composition's exact extent (`ITfCompositionView` → `ITfRangeACP`); a
//!   composition that ended unmarks the text (which commits it).
//! - **App edits** (typing outside the IME, undo, programmatic changes) reach TSF through
//!   [`TsfDocument::sync`], which compares the client with the last state TSF saw and sends
//!   `OnTextChange`/`OnSelectionChange`; field moves send `OnLayoutChange`.
//! - **Input scopes** (number, URL, password, …) are the `GUID_PROP_INPUTSCOPE` attribute, an
//!   `ITfInputScope` object.

use crate::util::utf16_len;
use skelter_base::geom::{Point, Rect};
use skelter_base::ids::WindowId;
use skelter_base::input::{InputPurpose, TextInputClient, Utf16Range};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use windows::Win32::Foundation::{
    E_FAIL, E_INVALIDARG, E_NOTIMPL, HWND, POINT, RECT, S_OK,
};
use windows::Win32::Graphics::Gdi::{ClientToScreen, ScreenToClient};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemAlloc, IDataObject, FORMATETC};
use windows::Win32::System::Ole::{CONNECT_E_ADVISELIMIT, CONNECT_E_NOCONNECTION};
use windows::Win32::System::Variant::{VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_UNKNOWN};
use windows::Win32::UI::TextServices::*;
use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, MSG, WM_KEYDOWN, WM_KEYUP};
use windows_core::{BOOL, BSTR, GUID, HRESULT, IUnknown, Interface, PCWSTR, PWSTR, implement};

/// The one view cookie the store reports.
const VIEW: u32 = 1;

/// Reads and edits the focused field of a window; `None` when the app has no text field there or
/// its handler is busy. Provided by the event loop.
pub type ClientAccess = Rc<dyn Fn(WindowId, &mut dyn FnMut(&mut dyn TextInputClient)) -> bool>;

/// The thread's TSF manager.
pub struct Tsf {
    thread_mgr: ITfThreadMgr,
    client_id: u32,
    keystrokes: Option<ITfKeystrokeMgr>,
    /// Focused when no text field is: the IME is off.
    empty_doc: ITfDocumentMgr,
}

impl Tsf {
    pub fn new() -> windows_core::Result<Self> {
        let thread_mgr: ITfThreadMgr = unsafe { CoCreateInstance(&CLSID_TF_ThreadMgr, None, CLSCTX_INPROC_SERVER) }?;
        let client_id = unsafe { thread_mgr.Activate() }?;
        let keystrokes = thread_mgr.cast::<ITfKeystrokeMgr>().ok();
        let empty_doc = unsafe { thread_mgr.CreateDocumentMgr() }?;
        Ok(Self { thread_mgr, client_id, keystrokes, empty_doc })
    }

    /// Lets the active TIP see a key before the window does; `true` when the TIP ate it (the
    /// message must not be dispatched).
    pub fn filter_key(&self, msg: &MSG) -> bool {
        let Some(k) = &self.keystrokes else { return false };
        unsafe {
            match msg.message {
                WM_KEYDOWN => {
                    if k.TestKeyDown(msg.wParam, msg.lParam).is_ok_and(|b| b.as_bool()) {
                        return k.KeyDown(msg.wParam, msg.lParam).is_ok_and(|b| b.as_bool());
                    }
                    false
                }
                WM_KEYUP => {
                    if k.TestKeyUp(msg.wParam, msg.lParam).is_ok_and(|b| b.as_bool()) {
                        return k.KeyUp(msg.wParam, msg.lParam).is_ok_and(|b| b.as_bool());
                    }
                    false
                }
                _ => false,
            }
        }
    }

    pub fn create_document(&self, hwnd: HWND, window: WindowId, access: ClientAccess) -> windows_core::Result<TsfDocument> {
        let state = Rc::new(StoreState::new(hwnd, window, access));
        let store: ITextStoreACP = TextStore { state: state.clone() }.into();
        let doc_mgr = unsafe { self.thread_mgr.CreateDocumentMgr() }?;
        let mut context: Option<ITfContext> = None;
        let mut edit_cookie = 0u32;
        unsafe { doc_mgr.CreateContext(self.client_id, 0, &store, &mut context, &mut edit_cookie) }?;
        let context = context.ok_or_else(|| windows_core::Error::from(E_FAIL))?;
        unsafe { doc_mgr.Push(&context) }?;
        Ok(TsfDocument { doc_mgr, context, _store: store, state })
    }

    /// Gives the keyboard's TSF focus to `doc`, or to nothing (IME off).
    pub fn focus(&self, doc: Option<&TsfDocument>) {
        let target = doc.map(|d| &d.doc_mgr).unwrap_or(&self.empty_doc);
        let _ = unsafe { self.thread_mgr.SetFocus(target) };
    }
}

impl Drop for Tsf {
    fn drop(&mut self) {
        let _ = unsafe { self.thread_mgr.Deactivate() };
    }
}

/// One window's TSF document: a document manager with one context over the window's text field.
pub struct TsfDocument {
    doc_mgr: ITfDocumentMgr,
    context: ITfContext,
    _store: ITextStoreACP,
    state: Rc<StoreState>,
}

impl TsfDocument {
    pub fn set_scale(&self, scale: f32) {
        self.state.scale.set(scale);
    }

    /// The field's caret or clause area and purpose; a moved area tells the TIP to re-place its
    /// candidate window.
    pub fn set_field(&self, area: Rect, purpose: InputPurpose) {
        let moved = self.state.area.get() != area;
        self.state.area.set(area);
        self.state.purpose.set(purpose);
        if moved {
            self.state.notify_layout();
        }
    }

    /// Tells TSF about edits the app made itself since TSF last looked.
    pub fn sync(&self) {
        self.state.sync();
    }

    /// Grants a lock TSF asked for while the handler was busy.
    pub fn grant_pending(&self) {
        self.state.grant_pending();
    }

    /// Ends the composition from the app: the TIP terminates it, which unmarks (commits) whatever
    /// marked text the field still has.
    pub fn end_composition(&self) {
        if let Ok(services) = self.context.cast::<ITfContextOwnerCompositionServices>() {
            let _ = unsafe { services.TerminateComposition(None::<&ITfCompositionView>) };
        }
    }
}

impl Drop for TsfDocument {
    fn drop(&mut self) {
        let _ = unsafe { self.doc_mgr.Pop(TF_POPF_ALL) };
    }
}

/// What a `RequestLock` call does now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockDecision {
    /// Call `OnLockGranted` with these flags (`TS_LF_READ` or `TS_LF_READWRITE`).
    Grant(u32),
    /// Queued; `TS_S_ASYNC`.
    Queued,
    /// `TS_E_SYNCHRONOUS`: a synchronous request that cannot be granted now.
    Refused,
}

/// The lock protocol: one lock at a time, one queued request (a write request supersedes a read).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LockState {
    /// The lock currently held (0 = none).
    pub held: u32,
    pub queued: Option<u32>,
}

impl LockState {
    pub fn request(&mut self, flags: u32, client_ready: bool) -> LockDecision {
        let kind = flags & TS_LF_READWRITE.0;
        let sync = flags & TS_LF_SYNC != 0;
        if self.held == 0 && client_ready {
            return LockDecision::Grant(kind);
        }
        if sync {
            return LockDecision::Refused;
        }
        self.queued = Some(match self.queued {
            Some(q) if q & TS_LF_READWRITE.0 == TS_LF_READWRITE.0 => q,
            _ => kind,
        });
        LockDecision::Queued
    }

    pub fn can_read(&self) -> bool {
        self.held & TS_LF_READ.0 != 0
    }

    pub fn can_write(&self) -> bool {
        self.held & TS_LF_READWRITE.0 == TS_LF_READWRITE.0
    }
}

/// A TSF `[start, end)` pair (`end` -1 = the document's end) checked against the document length.
pub fn resolve_range(start: i32, end: i32, len: usize) -> Result<Utf16Range, HRESULT> {
    let end = if end == -1 { len as i64 } else { end as i64 };
    let start = start as i64;
    if start < 0 || start > end || end > len as i64 {
        return Err(TS_E_INVALIDPOS);
    }
    Ok(start as usize..end as usize)
}

/// The `TS_TEXTCHANGE` for replacing `old` with `new_len` units.
pub fn text_change(old: &Utf16Range, new_len: usize) -> TS_TEXTCHANGE {
    TS_TEXTCHANGE { acpStart: old.start as i32, acpOldEnd: old.end as i32, acpNewEnd: (old.start + new_len) as i32 }
}

/// `ITfRangeACP::GetExtent`'s anchor and length as a range.
pub fn extent_range(anchor: i32, cch: i32) -> Utf16Range {
    let a = anchor.max(0) as usize;
    a..a + cch.max(0) as usize
}

/// A selection relative to the start of the marked text, clamped into it (what
/// `replace_and_mark_text_in_range` takes).
pub fn relative_selection(sel: &Utf16Range, marked: &Utf16Range) -> Utf16Range {
    let len = marked.end - marked.start;
    let rel = |x: usize| x.saturating_sub(marked.start).min(len);
    rel(sel.start)..rel(sel.end).max(rel(sel.start))
}

/// The input scope TSF gets for a field purpose.
pub fn input_scope(purpose: InputPurpose) -> InputScope {
    match purpose {
        InputPurpose::Text => IS_DEFAULT,
        InputPurpose::Number => IS_NUMBER,
        InputPurpose::Password => IS_PASSWORD,
        InputPurpose::Url => IS_URL,
        InputPurpose::Email => IS_EMAIL_SMTPEMAILADDRESS,
        InputPurpose::Phone => IS_TELEPHONE_FULLTELEPHONENUMBER,
        InputPurpose::Code => IS_ALPHANUMERIC_HALFWIDTH,
        InputPurpose::Search => IS_SEARCH,
    }
}

/// The document length: clients clamp `text_for_range` to the document, so asking for everything
/// returns the whole text. (A `len_utf16` on the client would avoid the copy; see the lane record.)
fn doc_len(c: &dyn TextInputClient) -> usize {
    c.text_for_range(0..usize::MAX).map(|s| utf16_len(&s)).unwrap_or(0)
}

/// What TSF last saw of the document, to detect the app's own edits cheaply (the selection, the
/// marked range and the text around the caret; the length is read only when one changed).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Snapshot {
    selection: Option<Utf16Range>,
    marked: Option<Utf16Range>,
    around: Option<String>,
}

fn snapshot(c: &dyn TextInputClient) -> Snapshot {
    Snapshot { selection: c.selected_range(), marked: c.marked_range(), around: c.surrounding_text(512).map(|(s, _)| s) }
}

struct StoreState {
    hwnd: HWND,
    window: WindowId,
    access: ClientAccess,
    scale: Cell<f32>,
    area: Cell<Rect>,
    purpose: Cell<InputPurpose>,
    sink: RefCell<Option<(ITextStoreACPSink, u32)>>,
    lock: Cell<LockState>,
    composition: RefCell<Option<ITfCompositionView>>,
    /// The composition ended during a lock; unmark when it is released.
    composition_ended: Cell<bool>,
    /// A selection TSF set outside a composition, which the client cannot take (no setter on
    /// `TextInputClient`): reported back to TSF until the document changes.
    selection_override: RefCell<Option<Utf16Range>>,
    seen: RefCell<Snapshot>,
    /// The document length TSF last saw.
    seen_len: Cell<usize>,
    /// `GUID_PROP_INPUTSCOPE` was requested and not yet retrieved.
    scope_requested: Cell<bool>,
}

impl StoreState {
    fn new(hwnd: HWND, window: WindowId, access: ClientAccess) -> Self {
        Self {
            hwnd,
            window,
            access,
            scale: Cell::new(1.0),
            area: Cell::new(Rect::ZERO),
            purpose: Cell::new(InputPurpose::Text),
            sink: RefCell::new(None),
            lock: Cell::new(LockState::default()),
            composition: RefCell::new(None),
            composition_ended: Cell::new(false),
            selection_override: RefCell::new(None),
            seen: RefCell::new(Snapshot::default()),
            seen_len: Cell::new(0),
            scope_requested: Cell::new(false),
        }
    }

    fn with_client<R>(&self, f: impl FnOnce(&mut dyn TextInputClient) -> R) -> Option<R> {
        let mut f = Some(f);
        let mut out = None;
        (self.access)(self.window, &mut |c| {
            if let Some(f) = f.take() {
                out = Some(f(c));
            }
        });
        out
    }

    fn client_ready(&self) -> bool {
        self.with_client(|_| ()).is_some()
    }

    fn sink(&self) -> Option<(ITextStoreACPSink, u32)> {
        self.sink.borrow().clone()
    }

    fn composing(&self) -> bool {
        self.composition.borrow().is_some()
    }

    fn request_lock(&self, flags: u32) -> HRESULT {
        let mut lock = self.lock.get();
        let decision = lock.request(flags, self.client_ready());
        self.lock.set(lock);
        match decision {
            LockDecision::Grant(kind) => self.run_lock(kind),
            LockDecision::Queued => TS_S_ASYNC,
            LockDecision::Refused => TS_E_SYNCHRONOUS,
        }
    }

    /// Holds the lock while the TIP works in `OnLockGranted`, then settles the client and runs a
    /// request queued meanwhile.
    fn run_lock(&self, kind: u32) -> HRESULT {
        let Some((sink, _)) = self.sink() else { return E_FAIL };
        self.lock.set(LockState { held: kind, ..self.lock.get() });
        let hr = match unsafe { sink.OnLockGranted(TEXT_STORE_LOCK_FLAGS(kind)) } {
            Ok(()) => S_OK,
            Err(e) => e.code(),
        };
        self.lock.set(LockState { held: 0, ..self.lock.get() });
        if kind == TS_LF_READWRITE.0 {
            self.settle_composition();
        }
        self.grant_pending();
        hr
    }

    fn grant_pending(&self) {
        let mut lock = self.lock.get();
        if lock.held != 0 || !self.client_ready() {
            return;
        }
        if let Some(kind) = lock.queued.take() {
            self.lock.set(lock);
            let _ = self.run_lock(kind);
        }
    }

    /// After a write lock: mark exactly the composition's extent, or unmark a finished one.
    fn settle_composition(&self) {
        let view = self.composition.borrow().clone();
        if let Some(view) = view {
            let extent = unsafe { view.GetRange() }.ok().and_then(|r| r.cast::<ITfRangeACP>().ok()).and_then(|r| {
                let (mut anchor, mut cch) = (0i32, 0i32);
                unsafe { r.GetExtent(&mut anchor, &mut cch) }.ok().map(|_| extent_range(anchor, cch))
            });
            if let Some(marked) = extent {
                self.with_client(|c| {
                    if c.marked_range().as_ref() != Some(&marked) {
                        let text = c.text_for_range(marked.clone()).unwrap_or_default();
                        let sel = c.selected_range().map(|s| relative_selection(&s, &marked));
                        c.replace_and_mark_text_in_range(Some(marked.clone()), &text, sel);
                    }
                });
            }
        }
        if self.composition_ended.replace(false) {
            self.with_client(|c| c.unmark_text());
        }
        if let Some((seen, len)) = self.with_client(|c| (snapshot(c), doc_len(c))) {
            *self.seen.borrow_mut() = seen;
            self.seen_len.set(len);
        }
    }

    fn sync(&self) {
        if self.lock.get().held != 0 {
            return;
        }
        let Some(now) = self.with_client(|c| snapshot(c)) else { return };
        let before = self.seen.replace(now.clone());
        if before == now {
            return;
        }
        let Some(len) = self.with_client(|c| doc_len(c)) else { return };
        let before_len = self.seen_len.replace(len);
        self.selection_override.borrow_mut().take();
        let Some((sink, mask)) = self.sink() else { return };
        let text_changed = before_len != len || before.around != now.around || before.marked != now.marked;
        unsafe {
            if text_changed && mask & TS_AS_TEXT_CHANGE != 0 {
                let change = TS_TEXTCHANGE { acpStart: 0, acpOldEnd: before_len as i32, acpNewEnd: len as i32 };
                let _ = sink.OnTextChange(TEXT_STORE_TEXT_CHANGE_FLAGS(0), &change);
            }
            if (text_changed || before.selection != now.selection) && mask & TS_AS_SEL_CHANGE != 0 {
                let _ = sink.OnSelectionChange();
            }
        }
    }

    fn notify_layout(&self) {
        if self.lock.get().held != 0 {
            return;
        }
        if let Some((sink, mask)) = self.sink()
            && mask & TS_AS_LAYOUT_CHANGE != 0
        {
            let _ = unsafe { sink.OnLayoutChange(TS_LC_CHANGE, VIEW) };
        }
    }

    fn selection(&self, c: &dyn TextInputClient) -> Utf16Range {
        if let Some(s) = self.selection_override.borrow().clone() {
            return s;
        }
        c.selected_range().unwrap_or(0..0)
    }

    /// Applies a TIP edit, keeping the text marked while a composition runs.
    fn edit(&self, c: &mut dyn TextInputClient, range: Utf16Range, text: &str) {
        self.selection_override.borrow_mut().take();
        if self.composing() {
            let n = utf16_len(text);
            c.replace_and_mark_text_in_range(Some(range), text, Some(n..n));
        } else {
            c.replace_text_in_range(Some(range), text);
        }
    }

    fn origin(&self) -> POINT {
        let mut p = POINT::default();
        let _ = unsafe { ClientToScreen(self.hwnd, &mut p) };
        p
    }

    /// Window points to a screen `RECT`.
    fn screen_rect(&self, r: Rect) -> RECT {
        let s = self.scale.get();
        let o = self.origin();
        RECT {
            left: o.x + (r.min.x * s).floor() as i32,
            top: o.y + (r.min.y * s).floor() as i32,
            right: o.x + (r.max.x * s).ceil() as i32,
            bottom: o.y + (r.max.y * s).ceil() as i32,
        }
    }
}

#[implement(ITextStoreACP, ITfContextOwnerCompositionSink)]
struct TextStore {
    state: Rc<StoreState>,
}

fn utf16_of(text: &PCWSTR, cch: u32) -> String {
    if text.is_null() || cch == 0 {
        return String::new();
    }
    let units = unsafe { std::slice::from_raw_parts(text.0, cch as usize) };
    String::from_utf16_lossy(units)
}

impl ITextStoreACP_Impl for TextStore_Impl {
    fn AdviseSink(&self, riid: *const GUID, punk: windows_core::Ref<IUnknown>, dwmask: u32) -> windows_core::Result<()> {
        if unsafe { *riid } != ITextStoreACPSink::IID {
            return Err(E_INVALIDARG.into());
        }
        let sink: ITextStoreACPSink = punk.ok()?.cast()?;
        let mut slot = self.state.sink.borrow_mut();
        match slot.as_mut() {
            Some((existing, mask)) if existing.cast::<IUnknown>()? == sink.cast::<IUnknown>()? => {
                *mask = dwmask;
                Ok(())
            }
            Some(_) => Err(CONNECT_E_ADVISELIMIT.into()),
            None => {
                *slot = Some((sink, dwmask));
                Ok(())
            }
        }
    }

    fn UnadviseSink(&self, punk: windows_core::Ref<IUnknown>) -> windows_core::Result<()> {
        let unk: IUnknown = punk.ok()?.cast()?;
        let mut slot = self.state.sink.borrow_mut();
        match slot.as_ref() {
            Some((existing, _)) if existing.cast::<IUnknown>()? == unk => {
                *slot = None;
                Ok(())
            }
            _ => Err(CONNECT_E_NOCONNECTION.into()),
        }
    }

    fn RequestLock(&self, dwlockflags: u32) -> windows_core::Result<HRESULT> {
        Ok(self.state.request_lock(dwlockflags))
    }

    fn GetStatus(&self) -> windows_core::Result<TS_STATUS> {
        Ok(TS_STATUS { dwDynamicFlags: 0, dwStaticFlags: TS_SS_NOHIDDENTEXT })
    }

    fn QueryInsert(&self, start: i32, end: i32, _cch: u32, result_start: *mut i32, result_end: *mut i32) -> windows_core::Result<()> {
        let len = self.state.with_client(|c| doc_len(c)).ok_or(E_FAIL)?;
        let r = resolve_range(start, end, len).map_err(windows_core::Error::from)?;
        unsafe {
            *result_start = r.start as i32;
            *result_end = r.end as i32;
        }
        Ok(())
    }

    fn GetSelection(&self, index: u32, count: u32, out: *mut TS_SELECTION_ACP, fetched: *mut u32) -> windows_core::Result<()> {
        if !self.state.lock.get().can_read() {
            return Err(TS_E_NOLOCK.into());
        }
        if index != TF_DEFAULT_SELECTION && index != 0 {
            return Err(E_INVALIDARG.into());
        }
        if count == 0 {
            unsafe { *fetched = 0 };
            return Ok(());
        }
        let sel = self.state.with_client(|c| self.state.selection(c)).ok_or(E_FAIL)?;
        unsafe {
            *out = TS_SELECTION_ACP {
                acpStart: sel.start as i32,
                acpEnd: sel.end as i32,
                style: TS_SELECTIONSTYLE { ase: TS_AE_END, fInterimChar: BOOL(0) },
            };
            *fetched = 1;
        }
        Ok(())
    }

    fn SetSelection(&self, count: u32, sel: *const TS_SELECTION_ACP) -> windows_core::Result<()> {
        if !self.state.lock.get().can_write() {
            return Err(TS_E_NOLOCK.into());
        }
        if count != 1 || sel.is_null() {
            return Err(E_INVALIDARG.into());
        }
        let s = unsafe { &*sel };
        let state = &self.state;
        state
            .with_client(|c| {
                let r = resolve_range(s.acpStart, s.acpEnd, doc_len(c))?;
                match c.marked_range() {
                    // Inside the composition the client takes the selection with the marked text.
                    Some(m) if r.start >= m.start && r.end <= m.end => {
                        let text = c.text_for_range(m.clone()).unwrap_or_default();
                        c.replace_and_mark_text_in_range(Some(m.clone()), &text, Some(relative_selection(&r, &m)));
                        state.selection_override.borrow_mut().take();
                    }
                    _ => *state.selection_override.borrow_mut() = Some(r),
                }
                Ok::<(), HRESULT>(())
            })
            .ok_or(E_FAIL)?
            .map_err(windows_core::Error::from)
    }

    fn GetText(
        &self,
        start: i32,
        end: i32,
        pchplain: PWSTR,
        cchplainreq: u32,
        pcchplainret: *mut u32,
        prgruninfo: *mut TS_RUNINFO,
        cruninforeq: u32,
        pcruninforet: *mut u32,
        pacpnext: *mut i32,
    ) -> windows_core::Result<()> {
        if !self.state.lock.get().can_read() {
            return Err(TS_E_NOLOCK.into());
        }
        let units: Vec<u16> = self
            .state
            .with_client(|c| {
                let r = resolve_range(start, end, doc_len(c))?;
                Ok::<_, HRESULT>(c.text_for_range(r).unwrap_or_default().encode_utf16().collect())
            })
            .ok_or(E_FAIL)?
            .map_err(windows_core::Error::from)?;
        let n = units.len().min(cchplainreq as usize);
        unsafe {
            if n > 0 && !pchplain.is_null() {
                std::ptr::copy_nonoverlapping(units.as_ptr(), pchplain.0, n);
            }
            *pcchplainret = n as u32;
            if cruninforeq > 0 && !prgruninfo.is_null() && n > 0 {
                *prgruninfo = TS_RUNINFO { uCount: n as u32, r#type: TS_RT_PLAIN };
                *pcruninforet = 1;
            } else {
                *pcruninforet = 0;
            }
            *pacpnext = start + n as i32;
        }
        Ok(())
    }

    fn SetText(&self, _flags: u32, start: i32, end: i32, text: &PCWSTR, cch: u32) -> windows_core::Result<TS_TEXTCHANGE> {
        if !self.state.lock.get().can_write() {
            return Err(TS_E_NOLOCK.into());
        }
        let text = utf16_of(text, cch);
        self.state
            .with_client(|c| {
                let r = resolve_range(start, end, doc_len(c))?;
                self.state.edit(c, r.clone(), &text);
                Ok::<_, HRESULT>(text_change(&r, cch as usize))
            })
            .ok_or(E_FAIL)?
            .map_err(windows_core::Error::from)
    }

    fn GetFormattedText(&self, _start: i32, _end: i32) -> windows_core::Result<IDataObject> {
        Err(E_NOTIMPL.into())
    }

    fn GetEmbedded(&self, _pos: i32, _service: *const GUID, _riid: *const GUID) -> windows_core::Result<IUnknown> {
        Err(E_NOTIMPL.into())
    }

    fn QueryInsertEmbedded(&self, _service: *const GUID, _format: *const FORMATETC) -> windows_core::Result<BOOL> {
        Ok(BOOL(0))
    }

    fn InsertEmbedded(&self, _flags: u32, _start: i32, _end: i32, _data: windows_core::Ref<IDataObject>) -> windows_core::Result<TS_TEXTCHANGE> {
        Err(E_NOTIMPL.into())
    }

    fn InsertTextAtSelection(
        &self,
        flags: u32,
        text: &PCWSTR,
        cch: u32,
        pacpstart: *mut i32,
        pacpend: *mut i32,
        pchange: *mut TS_TEXTCHANGE,
    ) -> windows_core::Result<()> {
        let query_only = flags & TF_IAS_QUERYONLY.0 != 0;
        let lock = self.state.lock.get();
        if !(if query_only { lock.can_read() } else { lock.can_write() }) {
            return Err(TS_E_NOLOCK.into());
        }
        let text = utf16_of(text, cch);
        let sel = self.state.with_client(|c| self.state.selection(c)).ok_or(E_FAIL)?;
        if query_only {
            unsafe {
                if !pacpstart.is_null() {
                    *pacpstart = sel.start as i32;
                }
                if !pacpend.is_null() {
                    *pacpend = sel.end as i32;
                }
            }
            return Ok(());
        }
        self.state.with_client(|c| self.state.edit(c, sel.clone(), &text)).ok_or(E_FAIL)?;
        let change = text_change(&sel, cch as usize);
        unsafe {
            if flags & TF_IAS_NOQUERY.0 == 0 {
                if !pacpstart.is_null() {
                    *pacpstart = change.acpStart;
                }
                if !pacpend.is_null() {
                    *pacpend = change.acpNewEnd;
                }
            }
            if !pchange.is_null() {
                *pchange = change;
            }
        }
        Ok(())
    }

    fn InsertEmbeddedAtSelection(
        &self,
        _flags: u32,
        _data: windows_core::Ref<IDataObject>,
        _start: *mut i32,
        _end: *mut i32,
        _change: *mut TS_TEXTCHANGE,
    ) -> windows_core::Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn RequestSupportedAttrs(&self, _flags: u32, count: u32, attrs: *const GUID) -> windows_core::Result<()> {
        let wanted = if attrs.is_null() { &[][..] } else { unsafe { std::slice::from_raw_parts(attrs, count as usize) } };
        self.state.scope_requested.set(wanted.contains(&GUID_PROP_INPUTSCOPE));
        Ok(())
    }

    fn RequestAttrsAtPosition(&self, _pos: i32, count: u32, attrs: *const GUID, flags: u32) -> windows_core::Result<()> {
        self.RequestSupportedAttrs(flags, count, attrs)
    }

    fn RequestAttrsTransitioningAtPosition(&self, _pos: i32, _count: u32, _attrs: *const GUID, _flags: u32) -> windows_core::Result<()> {
        self.state.scope_requested.set(false);
        Ok(())
    }

    fn FindNextAttrTransition(
        &self,
        _start: i32,
        halt: i32,
        _count: u32,
        _attrs: *const GUID,
        _flags: u32,
        pacpnext: *mut i32,
        pffound: *mut BOOL,
        plfoundoffset: *mut i32,
    ) -> windows_core::Result<()> {
        // The input scope covers the whole document: no transitions.
        unsafe {
            *pacpnext = halt;
            *pffound = BOOL(0);
            *plfoundoffset = 0;
        }
        Ok(())
    }

    fn RetrieveRequestedAttrs(&self, count: u32, vals: *mut TS_ATTRVAL, fetched: *mut u32) -> windows_core::Result<()> {
        if count == 0 || vals.is_null() || !self.state.scope_requested.replace(false) {
            unsafe { *fetched = 0 };
            return Ok(());
        }
        let scope: ITfInputScope = Scope { scope: input_scope(self.state.purpose.get()) }.into();
        let unk: IUnknown = scope.cast()?;
        unsafe {
            *vals = TS_ATTRVAL {
                idAttr: GUID_PROP_INPUTSCOPE,
                dwOverlapId: 0,
                varValue: variant_unknown(unk),
            };
            *fetched = 1;
        }
        Ok(())
    }

    fn GetEndACP(&self) -> windows_core::Result<i32> {
        if !self.state.lock.get().can_read() {
            return Err(TS_E_NOLOCK.into());
        }
        self.state.with_client(|c| doc_len(c) as i32).ok_or_else(|| E_FAIL.into())
    }

    fn GetActiveView(&self) -> windows_core::Result<u32> {
        Ok(VIEW)
    }

    fn GetACPFromPoint(&self, _view: u32, pt: *const POINT, _flags: u32) -> windows_core::Result<i32> {
        let mut p = unsafe { *pt };
        let _ = unsafe { ScreenToClient(self.state.hwnd, &mut p) };
        let s = self.state.scale.get();
        let local = Point::new(p.x as f32 / s, p.y as f32 / s);
        self.state
            .with_client(|c| c.character_index_for_point(local))
            .flatten()
            .map(|i| i as i32)
            .ok_or_else(|| TS_E_INVALIDPOINT.into())
    }

    fn GetTextExt(&self, _view: u32, start: i32, end: i32, prc: *mut RECT, pfclipped: *mut BOOL) -> windows_core::Result<()> {
        if !self.state.lock.get().can_read() {
            return Err(TS_E_NOLOCK.into());
        }
        let bounds = self
            .state
            .with_client(|c| resolve_range(start, end, doc_len(c)).ok().and_then(|r| c.bounds_for_range(r)))
            .flatten();
        // Some TIPs mishandle TS_E_NOLAYOUT (Firefox's MaybeHackNoErrorLayoutBugs): fall back to the
        // field's caret area.
        let rect = bounds.unwrap_or_else(|| self.state.area.get());
        unsafe {
            *prc = self.state.screen_rect(rect);
            *pfclipped = BOOL(0);
        }
        Ok(())
    }

    fn GetScreenExt(&self, _view: u32) -> windows_core::Result<RECT> {
        let mut r = RECT::default();
        unsafe { GetClientRect(self.state.hwnd, &mut r) }?;
        let o = self.state.origin();
        Ok(RECT { left: o.x, top: o.y, right: o.x + r.right, bottom: o.y + r.bottom })
    }

    fn GetWnd(&self, _view: u32) -> windows_core::Result<HWND> {
        Ok(self.state.hwnd)
    }
}

impl ITfContextOwnerCompositionSink_Impl for TextStore_Impl {
    fn OnStartComposition(&self, view: windows_core::Ref<ITfCompositionView>) -> windows_core::Result<BOOL> {
        *self.state.composition.borrow_mut() = view.ok().ok().cloned();
        self.state.composition_ended.set(false);
        Ok(BOOL(1))
    }

    fn OnUpdateComposition(&self, view: windows_core::Ref<ITfCompositionView>, _new: windows_core::Ref<ITfRange>) -> windows_core::Result<()> {
        *self.state.composition.borrow_mut() = view.ok().ok().cloned();
        Ok(())
    }

    fn OnEndComposition(&self, _view: windows_core::Ref<ITfCompositionView>) -> windows_core::Result<()> {
        self.state.composition.borrow_mut().take();
        if self.state.lock.get().held != 0 {
            self.state.composition_ended.set(true);
        } else {
            self.state.with_client(|c| c.unmark_text());
        }
        Ok(())
    }
}

/// A `VT_UNKNOWN` variant owning `unk` (the receiver clears it).
fn variant_unknown(unk: IUnknown) -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: std::mem::ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_UNKNOWN,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 { punkVal: std::mem::ManuallyDrop::new(Some(unk)) },
            }),
        },
    }
}

#[implement(ITfInputScope)]
struct Scope {
    scope: InputScope,
}

impl ITfInputScope_Impl for Scope_Impl {
    fn GetInputScopes(&self, out: *mut *mut InputScope, count: *mut u32) -> windows_core::Result<()> {
        let mem = unsafe { CoTaskMemAlloc(std::mem::size_of::<InputScope>()) } as *mut InputScope;
        if mem.is_null() {
            return Err(windows::Win32::Foundation::E_OUTOFMEMORY.into());
        }
        unsafe {
            *mem = self.scope;
            *out = mem;
            *count = 1;
        }
        Ok(())
    }

    fn GetPhrase(&self, _phrases: *mut *mut BSTR, _count: *mut u32) -> windows_core::Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn GetRegularExpression(&self) -> windows_core::Result<BSTR> {
        Err(E_NOTIMPL.into())
    }

    fn GetSRGS(&self) -> windows_core::Result<BSTR> {
        Err(E_NOTIMPL.into())
    }

    fn GetXML(&self) -> windows_core::Result<BSTR> {
        Err(E_NOTIMPL.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_resolve_against_the_document() {
        assert_eq!(resolve_range(0, -1, 10), Ok(0..10));
        assert_eq!(resolve_range(3, 7, 10), Ok(3..7));
        assert_eq!(resolve_range(10, 10, 10), Ok(10..10));
        assert_eq!(resolve_range(7, 3, 10), Err(TS_E_INVALIDPOS));
        assert_eq!(resolve_range(0, 11, 10), Err(TS_E_INVALIDPOS));
        assert_eq!(resolve_range(-2, 3, 10), Err(TS_E_INVALIDPOS));
    }

    #[test]
    fn text_changes_and_extents() {
        let c = text_change(&(2..5), 4);
        assert_eq!((c.acpStart, c.acpOldEnd, c.acpNewEnd), (2, 5, 6));
        assert_eq!(extent_range(4, 3), 4..7);
        assert_eq!(extent_range(-1, -1), 0..0);
    }

    #[test]
    fn selections_relative_to_the_composition() {
        assert_eq!(relative_selection(&(7..7), &(5..9)), 2..2);
        assert_eq!(relative_selection(&(3..12), &(5..9)), 0..4, "clamped into the marked text");
        assert_eq!(relative_selection(&(9..9), &(5..9)), 4..4);
    }

    #[test]
    fn lock_protocol() {
        let mut l = LockState::default();
        assert_eq!(l.request(TS_LF_READWRITE.0 | TS_LF_SYNC, true), LockDecision::Grant(TS_LF_READWRITE.0));
        l.held = TS_LF_READ.0;
        assert!(l.can_read() && !l.can_write());
        assert_eq!(l.request(TS_LF_READ.0 | TS_LF_SYNC, true), LockDecision::Refused, "re-entrant sync");
        assert_eq!(l.request(TS_LF_READ.0, true), LockDecision::Queued);
        assert_eq!(l.request(TS_LF_READWRITE.0, true), LockDecision::Queued);
        assert_eq!(l.queued, Some(TS_LF_READWRITE.0), "write supersedes read");
        assert_eq!(l.request(TS_LF_READ.0, true), LockDecision::Queued);
        assert_eq!(l.queued, Some(TS_LF_READWRITE.0), "and is not downgraded");
        let mut busy = LockState::default();
        assert_eq!(busy.request(TS_LF_READ.0 | TS_LF_SYNC, false), LockDecision::Refused, "handler busy");
        assert_eq!(busy.request(TS_LF_READ.0, false), LockDecision::Queued);
    }

    #[test]
    fn purposes_have_input_scopes() {
        assert_eq!(input_scope(InputPurpose::Number), IS_NUMBER);
        assert_eq!(input_scope(InputPurpose::Password), IS_PASSWORD);
        assert_eq!(input_scope(InputPurpose::Text), IS_DEFAULT);
    }
}
