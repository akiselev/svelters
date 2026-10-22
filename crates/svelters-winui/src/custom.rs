//! Hand-written converters named by `custom:` in the overrides.
use crate::bindings::Microsoft::UI::Xaml::Controls::{ColumnDefinition, ContentDialog, Grid, RowDefinition};
use crate::bindings::Microsoft::UI::Xaml::{GridLength, GridUnitType};
use crate::conv::{R, err};
use crate::parse::{self, GridLen};
use svelters_abi::Value;
use windows_collections::IVector;
use windows_core::Interface;

fn native(l: &GridLen) -> GridLength {
    GridLength { Value: l.value, GridUnitType: GridUnitType(l.unit) }
}

fn ours(l: GridLength) -> GridLen {
    GridLen { value: l.Value, unit: l.GridUnitType.0 }
}

fn lengths(v: &Value) -> R<Vec<GridLen>> {
    match v {
        Value::Null => Ok(Vec::new()),
        other => parse::grid_lengths(&parse::to_string(other)?),
    }
}

/// `row-definitions="Auto, *, 2*, 40"`: replaces the grid's RowDefinitions.
pub fn grid_rows_set(g: &Grid, v: &Value) -> R<()> {
    let ls = lengths(v)?;
    let rows = g.RowDefinitions().and_then(|r| r.cast::<IVector<RowDefinition>>()).map_err(err)?;
    rows.Clear().map_err(err)?;
    for l in &ls {
        let d = RowDefinition::new().map_err(err)?;
        d.SetHeight(native(l)).map_err(err)?;
        rows.Append(&d).map_err(err)?;
    }
    Ok(())
}

pub fn grid_rows_get(g: &Grid) -> R<Value> {
    let rows = g.RowDefinitions().and_then(|r| r.cast::<IVector<RowDefinition>>()).map_err(err)?;
    let n = rows.Size().map_err(err)?;
    let mut out = Vec::new();
    for i in 0..n {
        out.push(ours(rows.GetAt(i).map_err(err)?.Height().map_err(err)?));
    }
    Ok(Value::Str(parse::grid_lengths_text(&out)))
}

pub fn grid_columns_set(g: &Grid, v: &Value) -> R<()> {
    let ls = lengths(v)?;
    let cols = g.ColumnDefinitions().and_then(|c| c.cast::<IVector<ColumnDefinition>>()).map_err(err)?;
    cols.Clear().map_err(err)?;
    for l in &ls {
        let d = ColumnDefinition::new().map_err(err)?;
        d.SetWidth(native(l)).map_err(err)?;
        cols.Append(&d).map_err(err)?;
    }
    Ok(())
}

/// `IContentDialog` (winmd GUID; `ExclusiveTo(ContentDialog)`) and the vtable slot of its `ShowAsync()`: IUnknown and
/// IInspectable take slots 0-5, then the interface's methods in winmd order (`Hide` 54, `ShowAsync()` 55,
/// `ShowAsync(placement)` 56).
const ICONTENT_DIALOG: u128 = 0xac2145a3_4a32_5305_a81d_47509515bfce;
const SHOW_ASYNC_SLOT: u16 = 55;

/// `show-async()` of `<content-dialog>`: ContentDialog.ShowAsync(), fire and forget. A dialog needs a XamlRoot: one
/// inserted under a mounted element has it; otherwise the first window's is used. The returned
/// `IAsyncOperation<ContentDialogResult>` is released at once (no Promise yet: element methods have no async
/// completion slot); the result arrives with the `closed` event (`result`), the buttons raise
/// `primary-button-click` / `secondary-button-click` / `close-button-click`. Called through the vtable
/// (`types::rt::vcall`) because windows-bindgen would need the windows-future crate for the async type.
pub fn content_dialog_show(d: &ContentDialog, args: &[Value]) -> R<Value> {
    crate::parse::arity(args, 0)?;
    let el = d.cast::<crate::bindings::Microsoft::UI::Xaml::UIElement>().map_err(err)?;
    if el.XamlRoot().ok().is_none() {
        let root = crate::backend::first_xaml_root().ok_or("show-async: no activated window to show the dialog over")?;
        el.SetXamlRoot(&root).map_err(err)?;
    }
    let unk = d.cast::<windows_core::IUnknown>().map_err(err)?;
    let iface = crate::types::rt::qi(&unk, &windows_core::GUID::from_u128(ICONTENT_DIALOG)).map_err(err)?;
    let mut op: *mut std::ffi::c_void = std::ptr::null_mut();
    // SAFETY: `HRESULT ShowAsync(IContentDialog*, IAsyncOperation<ContentDialogResult>** result)`; one out pointer.
    unsafe { crate::types::rt::vcall(&iface, SHOW_ASYNC_SLOT, &[crate::types::invoke::Word::ptr(&mut op)]) }.map_err(err)?;
    if !op.is_null() {
        // SAFETY: an owned reference returned through the out parameter.
        drop(unsafe { windows_core::IUnknown::from_raw(op) });
    }
    Ok(Value::Null)
}

pub fn grid_columns_get(g: &Grid) -> R<Value> {
    let cols = g.ColumnDefinitions().and_then(|c| c.cast::<IVector<ColumnDefinition>>()).map_err(err)?;
    let n = cols.Size().map_err(err)?;
    let mut out = Vec::new();
    for i in 0..n {
        out.push(ours(cols.GetAt(i).map_err(err)?.Width().map_err(err)?));
    }
    Ok(Value::Str(parse::grid_lengths_text(&out)))
}
