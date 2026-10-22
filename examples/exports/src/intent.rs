//! RA2: `svelters:app/intent`: serde DTOs crossing as `Json<T>` (typed from their JSON Schema) and a
//! `#[js(tag)]` data enum. `app/src/intent.ts` narrows both unions; svelte-check type-checks it against
//! the dumped typings.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use svelters::Json;

/// Why a command cannot run (shaped like cadatomic's `Refusal`, which lives in a headless crate).
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
pub struct Refusal {
    /// The reason id (`args.missing`).
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<String>,
    pub message: String,
}

/// A command slot's state: serde's internally tagged form.
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SlotStatus {
    Empty,
    Filled { value: f64 },
    Refused(Refusal),
}

/// What a pick accepts: a svelters data enum.
#[derive(svelters::Enum, Clone, Debug, PartialEq)]
#[js(tag = "kind")]
pub enum Pick {
    Face { index: u32 },
    Edge { index: u32, both_sides: bool },
    Nothing,
}

/// The status of slot `slot` given `value`: refused when there is no value.
#[svelters::export(module = "intent")]
pub fn check(slot: String, value: Option<f64>) -> Json<SlotStatus> {
    Json(match value {
        Some(value) => SlotStatus::Filled { value },
        None => SlotStatus::Refused(Refusal { reason: "args.missing".into(), message: format!("{slot} needs a value"), slot: Some(slot) }),
    })
}

/// The refusal again, as a status (round-trips the serde value).
#[svelters::export(module = "intent")]
pub fn refused(r: Json<Refusal>) -> Json<SlotStatus> {
    Json(SlotStatus::Refused(r.0))
}

#[svelters::export(module = "intent")]
pub fn describe_pick(p: Pick) -> String {
    match p {
        Pick::Face { index } => format!("face {index}"),
        Pick::Edge { index, both_sides } => format!("edge {index}{}", if both_sides { " (both sides)" } else { "" }),
        Pick::Nothing => "nothing".into(),
    }
}
