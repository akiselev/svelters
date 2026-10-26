//! Text editing (lane TEXT-INPUT, `plans/cloud/presto-b2/LANE-text-input.md`): the `text-edit` kind
//! and the fields built on it, on **quire-text** (github.com/akiselev/quire), laid out with
//! skelter-text.
//!
//! | Module | What |
//! |---|---|
//! | [`editor`] | [`TextEditor`]: one field's quire-text document and view, keys, IME, pointer selection, focus rules, clipboard and PRIMARY requests, the `TextInputClient` operations |
//! | [`field`] | [`TextField`]: the `text-edit` kind's state: layout, scrolling, recording, the caret fast commit, signals, AccessKit, `impl TextInputClient` |
//! | [`kind`] | [`TextEdit`]: the `text-edit` kind (skelter-core `Kind`) over [`TextField`] |
//! | [`number`] | [`NumberField`]: the value boxes (number and quantity spinners), formats, steps, the commit seam |
//! | [`hex`] | [`HexField`]: the colour editor's hex field |
//! | [`cell`] | [`CellEditor`]: the table's cell editor |
//! | [`keys`] | the IME-first key route (architecture §11.2) and the per-platform key maps |
//! | [`policy`] | [`TextPolicy`]: per-platform field behaviour (FX1's text rows) |
//! | [`caret`] | the caret as a compositor fast commit |
//! | [`geometry`] | skelter-text's `TextLayout` as a field's geometry; the `TextSystem` layouter |
//! | [`lines`] | quire-text's `VisualLines` over a layout (Up/Down on wrapped rows) |
//! | [`runs`] | [`TextRanges`]: the shown text as accessibility runs (AccessKit `TextRun`s: rows, chunks, characters, word starts) |
//! | [`shown`] | the shown text (password mask, preedit) and its byte mapping |
//! | [`utf16`] | document-wide UTF-16 offsets over quire-text's `Text` |
//! | [`shim`] | stand-ins for wave-1 APIs not there yet (`TextLayout`, `TextSystem::layout`, `Clipboard`) |

pub mod caret;
pub mod cell;
pub mod editor;
pub mod field;
pub mod geometry;
pub mod hex;
pub mod kind;
pub mod keys;
pub mod lines;
pub mod number;
pub mod policy;
pub mod runs;
pub mod shim;
pub mod shown;
pub mod utf16;

pub use cell::{CellEditor, CellEnd};
pub use editor::{Changes, EditorConfig, FocusReason, TextEditor, TextRequest};
pub use field::{FieldColors, FieldSignal, TEXT_EDIT_PROPS, TEXT_EDIT_SIGNALS, TextField};
pub use hex::{HexField, HexSignal};
pub use kind::TextEdit;
pub use keys::{KeyAction, KeyRoute};
pub use number::{NumFormat, NumberField, NumberSignal, Steps};
pub use policy::{Platform, TextPolicy};
pub use runs::{TextRanges, TextRun};
