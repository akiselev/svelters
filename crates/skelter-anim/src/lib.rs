//! skelter's animation math (`plans/research/skelter/architecture.md` §12; design and constants in
//! `plans/research/skelter/notes/round2/animation-scroll.md`).
//!
//! Every curve is a pure function of time evaluated at a frame's predicted present time, never an
//! integration per frame, so the UI thread and the compositor compute the same value independently
//! and 60 Hz, 120 Hz and VRR look the same.
//!
//! | Module | What |
//! |---|---|
//! | [`easing`] | `cubic-bezier()`, `steps()`, `linear()`, and `svelte/easing` |
//! | [`interp`] | interpolation of numbers, lengths, colours and transform lists; matrix decomposition |
//! | [`timing`] | Web Animations timing and keyframe effects: [`sample`] |
//! | [`spring`] | closed-form springs and the Svelte 60 Hz mapping |
//! | [`scroll`] | wheel smooth-scroll curves, flings and the rubber band, per OS |
//! | [`css`] | the CSS value strings Svelte puts in keyframes |

pub mod css;
pub mod easing;
pub mod interp;
pub mod scroll;
pub mod spring;
pub mod timing;

use skelter_base::anim::{AnimProperty, Placement};

pub use timing::{Phase, Sample, retarget, sample};

/// Where a property's animation runs (architecture §12.1): opacity, transform, clip and scroll offset
/// on the compositor; colours, filters and stroke dash on the UI thread without layout; everything
/// else with style and layout per frame.
pub fn placement(p: AnimProperty) -> Placement {
    match p {
        AnimProperty::Opacity | AnimProperty::Transform | AnimProperty::Clip | AnimProperty::ScrollOffset => Placement::Compositor,
        AnimProperty::Color
        | AnimProperty::BackgroundColor
        | AnimProperty::BorderColor
        | AnimProperty::Filter
        | AnimProperty::StrokeDashOffset => Placement::Paint,
        AnimProperty::Width | AnimProperty::Height | AnimProperty::Css(_) => Placement::Layout,
    }
}
