//! Hand-written converters named by `custom:` in the overrides.
use crate::bindings::Microsoft::UI::Xaml::Controls::{ColumnDefinition, Grid, RowDefinition};
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

pub fn grid_columns_get(g: &Grid) -> R<Value> {
    let cols = g.ColumnDefinitions().and_then(|c| c.cast::<IVector<ColumnDefinition>>()).map_err(err)?;
    let n = cols.Size().map_err(err)?;
    let mut out = Vec::new();
    for i in 0..n {
        out.push(ours(cols.GetAt(i).map_err(err)?.Width().map_err(err)?));
    }
    Ok(Value::Str(parse::grid_lengths_text(&out)))
}
