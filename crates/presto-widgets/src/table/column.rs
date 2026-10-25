//! `table-column`: a column's props (data only; a column draws nothing itself) and the props'
//! spellings. The table reads its `table-column` children into a `Vec<ColumnState>` in child
//! order: a column's child index is its data column (`header-clicked`'s and `sort-column`'s
//! number) and, without a `name`, its key in `hidden-columns` and `header-state`.

use super::shim::{HAlign, Value};
use super::state::{as_bool, as_enum, as_f64, as_str};
use presto_base::change::ChangeClass;

/// The narrowest a column gets without a `min-width` (Qt's `minimumSectionSize` is 16-ish).
pub const MIN_COL: f32 = 16.0;
/// A column's width without a `width`.
pub const DEFAULT_WIDTH: f32 = 100.0;

pub const ALIGNS: &[&str] = &["start", "center", "end"];
pub const HEADER_ALIGNS: &[&str] = &["auto", "start", "center", "end"];
pub const SORTS: &[&str] = &["none", "ascending", "descending"];
pub const ELIDES: &[&str] = &["end", "none"];

/// One `table-column`'s props.
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnState {
    pub title: String,
    /// The `width` prop or the last user resize (unclamped; [`ColumnState::width`] clamps).
    pub width: Option<f32>,
    pub min_width: Option<f32>,
    pub max_width: Option<f32>,
    /// `visible` (`false` leaves it out of the shown columns, as `hidden-columns` does).
    pub visible: bool,
    pub resizable: bool,
    pub align: &'static str,
    /// `auto`: `align`.
    pub header_align: &'static str,
    /// The sort arrow the header shows while the table has no `sort-column`.
    pub sort: &'static str,
    pub editable: bool,
    /// Its name in `hidden-columns` and `header-state` (empty: its child index).
    pub name: String,
    pub hideable: bool,
    pub elide: &'static str,
    pub tooltip: String,
}

impl Default for ColumnState {
    fn default() -> Self {
        ColumnState {
            title: String::new(),
            width: None,
            min_width: None,
            max_width: None,
            visible: true,
            resizable: true,
            align: "start",
            header_align: "auto",
            sort: "none",
            editable: false,
            name: String::new(),
            hideable: true,
            elide: "end",
            tooltip: String::new(),
        }
    }
}

impl ColumnState {
    pub fn new(title: &str, width: f32) -> ColumnState {
        ColumnState { title: title.into(), width: Some(width), ..ColumnState::default() }
    }

    /// Writes prop `name`; the change class of the write.
    pub fn set_prop(&mut self, name: &str, v: &Value) -> Result<ChangeClass, String> {
        let opt = |v: &Value| -> Result<Option<f32>, String> {
            match v {
                Value::F64(f) if f.is_nan() => Ok(None),
                v => as_f64(v).map(|f| Some(f as f32)),
            }
        };
        match name {
            "title" => self.title = as_str(v)?,
            "width" => self.width = opt(v)?,
            "min-width" => self.min_width = opt(v)?,
            "max-width" => self.max_width = opt(v)?,
            "visible" => self.visible = as_bool(v)?,
            "resizable" => self.resizable = as_bool(v)?,
            "align" => self.align = as_enum(v, ALIGNS)?,
            "header-align" => self.header_align = as_enum(v, HEADER_ALIGNS)?,
            "sort" => self.sort = as_enum(v, SORTS)?,
            "editable" => self.editable = as_bool(v)?,
            "name" => self.name = as_str(v)?,
            "hideable" => self.hideable = as_bool(v)?,
            "elide" => self.elide = as_enum(v, ELIDES)?,
            "tooltip" => self.tooltip = as_str(v)?,
            _ => return Err(format!("{name} is not a prop of <table-column>")),
        }
        // every column prop changes the table's header or cells; widths and visibility its layout
        Ok(match name {
            "width" | "min-width" | "max-width" | "visible" => ChangeClass::Layout,
            "editable" | "hideable" | "name" | "tooltip" | "resizable" => ChangeClass::Property,
            _ => ChangeClass::Paint,
        })
    }

    pub fn get_prop(&self, name: &str) -> Option<Value> {
        let s = |x: &str| Value::Str(x.to_string());
        let f = |x: Option<f32>| Value::F64(x.map_or(f64::NAN, f64::from));
        Some(match name {
            "title" => s(&self.title),
            "width" => f(self.width),
            "min-width" => f(self.min_width),
            "max-width" => f(self.max_width),
            "visible" => Value::Bool(self.visible),
            "resizable" => Value::Bool(self.resizable),
            "align" => s(self.align),
            "header-align" => s(self.header_align),
            "sort" => s(self.sort),
            "editable" => Value::Bool(self.editable),
            "name" => s(&self.name),
            "hideable" => Value::Bool(self.hideable),
            "elide" => s(self.elide),
            "tooltip" => s(&self.tooltip),
            _ => return None,
        })
    }

    pub fn min(&self) -> f32 {
        self.min_width.unwrap_or(MIN_COL).max(1.0)
    }

    pub fn max(&self) -> f32 {
        self.max_width.unwrap_or(f32::INFINITY).max(self.min())
    }

    /// Its width, clamped to `min-width`..`max-width`.
    pub fn width(&self) -> f32 {
        self.width.unwrap_or(DEFAULT_WIDTH).clamp(self.min(), self.max())
    }

    /// Its key in `hidden-columns` and `header-state`: `name`, else child index `data`.
    pub fn key(&self, data: usize) -> String {
        if self.name.is_empty() { data.to_string() } else { self.name.clone() }
    }

    pub fn cell_align(&self) -> HAlign {
        halign(self.align)
    }

    pub fn head_align(&self) -> HAlign {
        if self.header_align == "auto" { halign(self.align) } else { halign(self.header_align) }
    }
}

fn halign(a: &str) -> HAlign {
    match a {
        "center" => HAlign::Center,
        "end" => HAlign::End,
        _ => HAlign::Start,
    }
}
