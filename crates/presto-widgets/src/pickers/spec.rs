//! The pickers' spec rows (svelters-egui `kinds/pickers.rs`): tag, props (name, type, default,
//! change class) and signals (name, argument names) of `calendar`, `color-editor`, `date-picker`
//! and `color-picker`, in the order the svelters spec lists them. The `KindSpec`s are built from
//! these once presto-core's `KindSpec` exists.

use presto_base::change::ChangeClass;

/// A prop's JS-facing type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Str,
    Bool,
    StrList,
    /// One of these strings.
    Enum(&'static [&'static str]),
}

/// One prop row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PropRow {
    pub name: &'static str,
    pub ty: Ty,
    /// The default as JS would write it (`""`, `"auto"`, `true`; `null` for none).
    pub default: &'static str,
    pub change: ChangeClass,
    /// `bind:`able: the kind writes it back (`input` rows of svelters-egui).
    pub bind: bool,
}

/// One signal row: name and argument names.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SignalRow {
    pub name: &'static str,
    pub args: &'static [&'static str],
}

/// One kind's rows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KindRows {
    pub tag: &'static str,
    pub props: &'static [PropRow],
    pub signals: &'static [SignalRow],
}

const fn p(name: &'static str, ty: Ty, default: &'static str, change: ChangeClass) -> PropRow {
    PropRow { name, ty, default, change, bind: false }
}
const fn bind(name: &'static str, ty: Ty, default: &'static str, change: ChangeClass) -> PropRow {
    PropRow { name, ty, default, change, bind: true }
}

use super::calendar::{FIRST_DAY, WEEKDAY_FORMAT};
use super::color::ALPHA;
use super::color_editor::COLOR_COMMIT_MODE;
use super::date_picker::DATE_COMMIT_MODE;
use ChangeClass::{Layout, Paint};

const CAL_MIN: PropRow = p("min", Ty::Str, "\"\"", Paint);
const CAL_MAX: PropRow = p("max", Ty::Str, "\"\"", Paint);
const FIRST: PropRow = p("first-day", Ty::Enum(FIRST_DAY), "\"auto\"", Paint);
const WD_FORMAT: PropRow = p("weekday-format", Ty::Enum(WEEKDAY_FORMAT), "\"short\"", Layout);
const NAVIGATION: PropRow = p("navigation", Ty::Bool, "true", Layout);
const READ_ONLY: PropRow = p("read-only", Ty::Bool, "false", Paint);
const DAY_PARTS: PropRow = p("day-parts", Ty::StrList, "null", Paint);
const FIELDS: PropRow = p("fields", Ty::Str, "\"hex rgb\"", Layout);
const SWATCHES: PropRow = p("swatches", Ty::StrList, "null", Layout);
const CUSTOM: PropRow = bind("custom-colors", Ty::StrList, "null", Layout);

const SIG_CHANGED: SignalRow = SignalRow { name: "changed", args: &["value"] };
const SIG_COMMIT: SignalRow = SignalRow { name: "commit", args: &["value"] };
const SIG_PAGE: SignalRow = SignalRow { name: "page-changed", args: &["year", "month"] };

pub const CALENDAR: KindRows = KindRows {
    tag: "calendar",
    props: &[bind("value", Ty::Str, "\"\"", Paint), p("calendar-week", Ty::Bool, "true", Layout), CAL_MIN, CAL_MAX, FIRST, WD_FORMAT, NAVIGATION, READ_ONLY, DAY_PARTS],
    signals: &[SIG_CHANGED, SIG_COMMIT, SIG_PAGE],
};

pub const DATE_PICKER: KindRows = KindRows {
    tag: "date-picker",
    props: &[
        bind("value", Ty::Str, "\"\"", Layout),
        p("format", Ty::Str, "\"%Y-%m-%d\"", Layout),
        p("calendar-week", Ty::Bool, "true", Paint),
        CAL_MIN,
        CAL_MAX,
        FIRST,
        p("weekday-format", Ty::Enum(WEEKDAY_FORMAT), "\"short\"", Paint),
        p("navigation", Ty::Bool, "true", Paint),
        READ_ONLY,
        DAY_PARTS,
        p("commit-mode", Ty::Enum(DATE_COMMIT_MODE), "\"confirm\"", Paint),
    ],
    signals: &[SIG_CHANGED, SIG_COMMIT, SIG_PAGE],
};

pub const COLOR_EDITOR: KindRows = KindRows {
    tag: "color-editor",
    props: &[bind("value", Ty::Str, "\"#000000\"", Paint), p("alpha", Ty::Enum(ALPHA), "\"opaque\"", Layout), FIELDS, SWATCHES, CUSTOM],
    signals: &[SIG_CHANGED, SIG_COMMIT],
};

pub const COLOR_PICKER: KindRows = KindRows {
    tag: "color-picker",
    props: &[bind("value", Ty::Str, "\"#000000\"", Paint), p("alpha", Ty::Enum(ALPHA), "\"opaque\"", Paint), FIELDS, SWATCHES, CUSTOM, p("commit-mode", Ty::Enum(COLOR_COMMIT_MODE), "\"live\"", Paint)],
    signals: &[SIG_CHANGED, SIG_COMMIT],
};

/// All four, for the spec generator.
pub const KINDS: [KindRows; 4] = [CALENDAR, COLOR_EDITOR, DATE_PICKER, COLOR_PICKER];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique_per_kind() {
        for k in KINDS {
            let mut names: Vec<&str> = k.props.iter().map(|p| p.name).collect();
            names.sort();
            names.dedup();
            assert_eq!(names.len(), k.props.len(), "{}", k.tag);
            assert_eq!(k.props[0].name, "value");
            assert!(k.props[0].bind);
        }
    }
}
