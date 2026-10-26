//! skelter's retained tree (architecture §4–§6, §11, §13).
//!
//! A [`Document`] holds every node of one app, across its windows, as columns indexed by node slot
//! (svelters-egui's scene arena, re-hosted). Each node has a [`Kind`]: a static [`KindSpec`] that
//! declares its props with their [`ChangeClass`](skelter_base::change::ChangeClass), its signals and
//! slots, and a value that measures, records, hit-tests, handles input and projects accessibility.
//! Edits mark exactly the work their class causes; [`Document::frame`] runs the UI phases for a
//! window and returns the [`Commit`](skelter_base::commit::Commit) delta for the compositor;
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
//! | [`layout`] | taffy on the tree, invalidation and the measure guard |
//! | [`frame`] | the UI phases and the commit builder |
//! | [`input`] | hit testing, dispatch, hover, capture, focus |
//! | [`hit`] | hit lists |
//! | [`journal`] | the change journal |
//! | [`query`] | kind sets, anchors, [`Query`](query::Query), CSS selector queries |
//! | [`a11y`] | the AccessKit projection and its validating builder |
//! | [`testkinds`] | minimal kinds and fixed-metric text for tests |
//! | [`virt`] | the row-height model and the row pool of virtualising kinds |
//! | `runner` | entries the app runner drives: `needs_frame`, `hover`, `accessibility_action`, `window_event`, `device_reset`, `ime_state` |
//! | [`settle`] | [`Busy`]: why the document is not idle |

pub mod a11y;
mod caret;
pub mod common;
mod compact;
pub mod cx;
mod document;
pub mod error;
pub mod frame;
pub mod hit;
pub mod ids;
pub mod input;
pub mod journal;
pub mod kind;
pub mod layout;
pub mod query;
mod record;
pub mod registry;
mod runner;
mod scroll;
pub mod settle;
pub mod style;
pub mod testkinds;
mod tree;
pub mod value;
pub mod virt;
mod window;
mod anim;
#[cfg(test)]
mod tests;

pub use cx::{Services, A11yCx, ChildLayoutCx, EventCx, RealizeCx, HitCx, MeasureCx, NodeView, PrepareCx, PropCx, RecordCx, ShapedText, StyleCx, TextShaper};
pub use document::{Document, DragPreviewProvider, SyncDelivery, Writer};
pub use error::{PropError, TreeError};
pub use frame::FrameStats;
pub use layout::LayoutRun;
pub use settle::Busy;
pub use ids::{KindId, MethodId, PropId, SignalId, SlotId, fingerprint};
pub use input::{Delivery, Dispatched, JsEvent, Phase};
pub use input::focus::{next_focus, open_dialog};
pub use kind::{ArgSpec, DEFAULT_NAME_FROM, DragPreview, Event, FocusReason, Handled, HitResult, Kind, KindSpec, MethodSpec, NameSource, NativeState, Outcome, PeerGroup, PeerMsg, PeerTarget, PointerInput, PropSpec, Rove, SelfScroll, SignalFlags, SignalSpec, SlotSpec, VirtualChildren, WheelInput};
pub use registry::{KindEntry, KindRegistry, RegistryError};
pub use style::{ComputedBox, StyleInput};
pub use tree::{Dirty, ListenFlags, Listener, NodeFlags};
pub use value::{PropDefault, Value, ValueType};
pub use skelter_base;
pub use svelters_style::StateMask;
pub use taffy;
