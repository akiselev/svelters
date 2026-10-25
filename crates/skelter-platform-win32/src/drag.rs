//! OLE drag and drop.
//!
//! - **Source:** [`run_drag`] runs `DoDragDrop` with a data object carrying Unicode text and file
//!   lists (`CF_HDROP`). `DoDragDrop` is a modal loop that keeps dispatching window messages; the
//!   event loop starts it only after the handler call that asked for it returned, so wakes, timers,
//!   resizes and frames keep reaching the app during the drag (winit starts it inside the handler,
//!   which leaves the window showing stale content).
//! - **Target:** a [`DropTarget`] per window turns Explorer file drags into
//!   `FileHovered`/`FileDropped`/`FileHoverCancelled` events.

use crate::clipboard::{clipboard_utf16, hdrop_paths};
use skelter_base::event::InputEvent;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use windows::Win32::Foundation::{
    DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS, DV_E_FORMATETC, E_NOTIMPL, E_OUTOFMEMORY, HGLOBAL,
    OLE_E_ADVISENOTSUPPORTED, POINTL, S_OK,
};
use windows::Win32::System::Com::{
    DATADIR_GET, DVASPECT_CONTENT, FORMATETC, IAdviseSink, IDataObject, IDataObject_Impl, IEnumFORMATETC,
    IEnumSTATDATA, STGMEDIUM, STGMEDIUM_0, TYMED_HGLOBAL,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GMEM_ZEROINIT, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::{
    CF_HDROP, CF_UNICODETEXT, DROPEFFECT,
    DROPEFFECT_COPY, DROPEFFECT_MOVE, DROPEFFECT_NONE, DoDragDrop, IDropSource, IDropSource_Impl, IDropTarget,
    IDropTarget_Impl, ReleaseStgMedium,
};
use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};
use windows::Win32::UI::Shell::{DROPFILES, HDROP, SHCreateStdEnumFmtEtc};
use windows_core::{BOOL, HRESULT, implement};

/// What an OS drag carries. (skelter-platform has no drag-source call yet; [`run_drag`] is ready for
/// one.)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DragData {
    pub text: Option<String>,
    pub files: Vec<PathBuf>,
    /// Allow the target to move instead of copy.
    pub allow_move: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragResult {
    Copied,
    Moved,
    Cancelled,
}

/// `DROPFILES` followed by the double-NUL-terminated wide path list.
pub fn hdrop_bytes(files: &[PathBuf]) -> Vec<u8> {
    let header = std::mem::size_of::<DROPFILES>();
    let mut out = Vec::new();
    for v in [header as u32, 0, 0, 0, 1] {
        // pFiles, pt.x, pt.y, fNC, fWide
        out.extend_from_slice(&v.to_le_bytes());
    }
    debug_assert_eq!(out.len(), header);
    for f in files {
        for u in f.to_string_lossy().encode_utf16().chain(std::iter::once(0)) {
            out.extend_from_slice(&u.to_le_bytes());
        }
    }
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn global_from(bytes: &[u8]) -> windows_core::Result<HGLOBAL> {
    let mem = unsafe { GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, bytes.len().max(1)) }?;
    let ptr = unsafe { GlobalLock(mem) } as *mut u8;
    if ptr.is_null() {
        return Err(E_OUTOFMEMORY.into());
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
        let _ = GlobalUnlock(mem);
    }
    Ok(mem)
}

fn format(cf: u16) -> FORMATETC {
    FORMATETC {
        cfFormat: cf,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    }
}

#[implement(IDataObject)]
struct DataObject {
    data: DragData,
}

impl DataObject {
    fn formats(&self) -> Vec<FORMATETC> {
        let mut f = Vec::new();
        if self.data.text.is_some() {
            f.push(format(CF_UNICODETEXT.0));
        }
        if !self.data.files.is_empty() {
            f.push(format(CF_HDROP.0));
        }
        f
    }

    fn bytes(&self, cf: u16) -> Option<Vec<u8>> {
        if cf == CF_UNICODETEXT.0 {
            let text = self.data.text.as_deref()?;
            Some(clipboard_utf16(text).iter().flat_map(|u| u.to_le_bytes()).collect())
        } else if cf == CF_HDROP.0 && !self.data.files.is_empty() {
            Some(hdrop_bytes(&self.data.files))
        } else {
            None
        }
    }

    fn supports(&self, f: &FORMATETC) -> bool {
        f.tymed & TYMED_HGLOBAL.0 as u32 != 0 && self.formats().iter().any(|g| g.cfFormat == f.cfFormat)
    }
}

impl IDataObject_Impl for DataObject_Impl {
    fn GetData(&self, pformatetcin: *const FORMATETC) -> windows_core::Result<STGMEDIUM> {
        let f = unsafe { &*pformatetcin };
        if !self.supports(f) {
            return Err(DV_E_FORMATETC.into());
        }
        let bytes = self.bytes(f.cfFormat).ok_or(DV_E_FORMATETC)?;
        let mem = global_from(&bytes)?;
        Ok(STGMEDIUM {
            tymed: TYMED_HGLOBAL.0 as u32,
            u: STGMEDIUM_0 { hGlobal: mem },
            pUnkForRelease: std::mem::ManuallyDrop::new(None),
        })
    }

    fn GetDataHere(&self, _f: *const FORMATETC, _m: *mut STGMEDIUM) -> windows_core::Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn QueryGetData(&self, pformatetc: *const FORMATETC) -> HRESULT {
        if self.supports(unsafe { &*pformatetc }) { S_OK } else { DV_E_FORMATETC }
    }

    fn GetCanonicalFormatEtc(&self, _in: *const FORMATETC, out: *mut FORMATETC) -> HRESULT {
        unsafe { (*out).ptd = std::ptr::null_mut() };
        E_NOTIMPL
    }

    fn SetData(&self, _f: *const FORMATETC, _m: *const STGMEDIUM, _release: BOOL) -> windows_core::Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn EnumFormatEtc(&self, direction: u32) -> windows_core::Result<IEnumFORMATETC> {
        if direction != DATADIR_GET.0 as u32 {
            return Err(E_NOTIMPL.into());
        }
        unsafe { SHCreateStdEnumFmtEtc(&self.formats()) }
    }

    fn DAdvise(&self, _f: *const FORMATETC, _advf: u32, _sink: windows_core::Ref<IAdviseSink>) -> windows_core::Result<u32> {
        Err(OLE_E_ADVISENOTSUPPORTED.into())
    }

    fn DUnadvise(&self, _connection: u32) -> windows_core::Result<()> {
        Err(OLE_E_ADVISENOTSUPPORTED.into())
    }

    fn EnumDAdvise(&self) -> windows_core::Result<IEnumSTATDATA> {
        Err(OLE_E_ADVISENOTSUPPORTED.into())
    }
}

#[implement(IDropSource)]
struct DropSource;

impl IDropSource_Impl for DropSource_Impl {
    fn QueryContinueDrag(&self, escape: BOOL, keys: MODIFIERKEYS_FLAGS) -> HRESULT {
        if escape.as_bool() {
            DRAGDROP_S_CANCEL
        } else if keys.0 & MK_LBUTTON.0 == 0 {
            DRAGDROP_S_DROP
        } else {
            S_OK
        }
    }

    fn GiveFeedback(&self, _effect: DROPEFFECT) -> HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}

/// What a finished `DoDragDrop` means.
pub fn drag_result(hr: HRESULT, effect: DROPEFFECT) -> DragResult {
    if hr != DRAGDROP_S_DROP {
        DragResult::Cancelled
    } else if effect.0 & DROPEFFECT_MOVE.0 != 0 {
        DragResult::Moved
    } else if effect.0 & DROPEFFECT_COPY.0 != 0 {
        DragResult::Copied
    } else {
        DragResult::Cancelled
    }
}

/// Runs an OLE drag to completion (a modal loop that keeps dispatching messages).
pub fn run_drag(data: DragData) -> DragResult {
    let allowed = if data.allow_move { DROPEFFECT(DROPEFFECT_COPY.0 | DROPEFFECT_MOVE.0) } else { DROPEFFECT_COPY };
    let object: IDataObject = DataObject { data }.into();
    let source: IDropSource = DropSource.into();
    let mut effect = DROPEFFECT_NONE;
    let hr = unsafe { DoDragDrop(&object, &source, allowed, &mut effect) };
    drag_result(hr, effect)
}

/// Turns file drags over a window into input events.
#[implement(IDropTarget)]
pub struct DropTarget {
    emit: Rc<dyn Fn(InputEvent)>,
    hovering: RefCell<Vec<PathBuf>>,
}

impl DropTarget {
    pub fn create(emit: Rc<dyn Fn(InputEvent)>) -> IDropTarget {
        DropTarget { emit, hovering: RefCell::new(Vec::new()) }.into()
    }
}

fn dropped_files(data: Option<&IDataObject>) -> Vec<PathBuf> {
    let Some(data) = data else { return Vec::new() };
    let f = format(CF_HDROP.0);
    let Ok(mut medium) = (unsafe { data.GetData(&f) }) else { return Vec::new() };
    let paths = hdrop_paths(HDROP(unsafe { medium.u.hGlobal }.0));
    unsafe { ReleaseStgMedium(&mut medium) };
    paths
}

#[allow(clippy::not_unsafe_ptr_arg_deref, reason = "COM methods receive the caller's out-pointers")]
impl IDropTarget_Impl for DropTarget_Impl {
    fn DragEnter(
        &self,
        data: windows_core::Ref<IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows_core::Result<()> {
        let files = dropped_files(data.as_ref());
        for f in &files {
            (self.emit)(InputEvent::FileHovered(f.clone()));
        }
        unsafe { *effect = if files.is_empty() { DROPEFFECT_NONE } else { DROPEFFECT_COPY } };
        *self.hovering.borrow_mut() = files;
        Ok(())
    }

    fn DragOver(&self, _keys: MODIFIERKEYS_FLAGS, _pt: &POINTL, effect: *mut DROPEFFECT) -> windows_core::Result<()> {
        unsafe { *effect = if self.hovering.borrow().is_empty() { DROPEFFECT_NONE } else { DROPEFFECT_COPY } };
        Ok(())
    }

    fn DragLeave(&self) -> windows_core::Result<()> {
        if !std::mem::take(&mut *self.hovering.borrow_mut()).is_empty() {
            (self.emit)(InputEvent::FileHoverCancelled);
        }
        Ok(())
    }

    fn Drop(
        &self,
        data: windows_core::Ref<IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows_core::Result<()> {
        self.hovering.borrow_mut().clear();
        let files = dropped_files(data.as_ref());
        for f in &files {
            (self.emit)(InputEvent::FileDropped(f.clone()));
        }
        unsafe { *effect = if files.is_empty() { DROPEFFECT_NONE } else { DROPEFFECT_COPY } };
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hdrop_layout() {
        let b = hdrop_bytes(&[PathBuf::from("C:\\a.txt"), PathBuf::from("D:\\b")]);
        let header = std::mem::size_of::<DROPFILES>();
        assert_eq!(header, 20);
        assert_eq!(u32::from_le_bytes(b[0..4].try_into().unwrap()) as usize, header, "pFiles");
        assert_eq!(u32::from_le_bytes(b[16..20].try_into().unwrap()), 1, "fWide");
        let units: Vec<u16> = b[header..].chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        let s = String::from_utf16(&units).unwrap();
        assert_eq!(s, "C:\\a.txt\0D:\\b\0\0");
    }

    #[test]
    fn drag_results() {
        assert_eq!(drag_result(DRAGDROP_S_DROP, DROPEFFECT_MOVE), DragResult::Moved);
        assert_eq!(drag_result(DRAGDROP_S_DROP, DROPEFFECT_COPY), DragResult::Copied);
        assert_eq!(drag_result(DRAGDROP_S_DROP, DROPEFFECT_NONE), DragResult::Cancelled);
        assert_eq!(drag_result(DRAGDROP_S_CANCEL, DROPEFFECT_COPY), DragResult::Cancelled);
    }
}
