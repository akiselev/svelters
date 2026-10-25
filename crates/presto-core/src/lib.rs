//! presto's retained tree (architecture §4–§6, §11, §13).
//!
//! A [`Document`] holds every node of one app, across its windows, as columns indexed by node slot
//! (svelters-egui's scene arena, re-hosted). Each node has a [`Kind`]: a static [`KindSpec`] that
//! declares its props with their [`ChangeClass`](presto_base::change::ChangeClass), its signals and
//! slots, and a value that measures, records, hit-tests, handles input and projects accessibility.
//! Edits mark exactly the work their class causes; [`Document::frame`] runs the UI phases for a
//! window and returns the [`Commit`](presto_base::commit::Commit) delta for the compositor;
//! [`Document::dispatch`] handles input at event time and lists the JS-facing events.
//!
//! | Module | What |
//! |---|---|
//! | [`ids`] | stable kind and member ids ([`KindId`], [`PropId`], [`SignalId`], [`SlotId`]) |
//! | [`value`] | prop values ([`Value`]) and their types |
//! | [`kind`] | [`Kind`], [`KindSpec`], [`PropSpec`], [`SignalSpec`], [`SlotSpec`], kind events |
//! | [`registry`] | [`KindRegistry`] |
//! | [`common`] | the props and DOM events every node has |
//! | [`cx`] | the contexts kinds are called with |
//! | `document` | [`Document`]: tree edits, props, listeners, style inputs, state, transactions |
//! | `tree` | the slot table, flags and dirty bits |
//! | [`style`] | the cascade (svelters-style) and the computed style |
//! | `layout` | taffy on the tree, invalidation and the measure guard |
//! | `frame` | the UI phases and the commit builder |
//! | [`input`] | hit testing, dispatch, hover, capture, focus |
//! | [`hit`] | hit lists |
//! | [`journal`] | the change journal |

pub mod common;
pub mod cx;
mod document;
pub mod error;
mod frame;
pub mod hit;
pub mod ids;
pub mod input;
pub mod journal;
pub mod kind;
mod layout;
mod record;
pub mod registry;
mod scroll;
pub mod style;
mod tree;
pub mod value;
mod window;
mod anim;

pub use cx::{A11yCx, EventCx, HitCx, MeasureCx, NodeView, PrepareCx, PropCx, RecordCx, ShapedText, StyleCx, TextShaper};
pub use document::{Document, Writer};
pub use error::{PropError, TreeError};
pub use ids::{KindId, PropId, SignalId, SlotId, fingerprint};
pub use input::{Delivery, Dispatched, JsEvent, Phase};
pub use input::focus::{next_focus, open_dialog};
pub use kind::{Event, FocusReason, Handled, HitResult, Kind, KindSpec, NativeState, PointerInput, PropSpec, SignalFlags, SignalSpec, SlotSpec, VirtualChildren, WheelInput};
pub use registry::{KindEntry, KindRegistry, RegistryError};
pub use style::{ComputedBox, StyleInput};
pub use tree::{Dirty, ListenFlags, Listener, NodeFlags};
pub use value::{PropDefault, Value, ValueType};
pub use presto_base;
pub use svelters_style::StateMask;
pub use taffy;
