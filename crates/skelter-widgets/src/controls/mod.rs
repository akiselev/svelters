//! Tier 0 kinds (EGUI.md §5): the layout containers (`box`, `frame`, `stack`, `sides`, `grid`), the
//! text and media leaves (`label` with rich runs, `hyperlink`, `icon`, `image`, `separator`,
//! `progress-bar`, `spinner`) and the controls (`button`, `toggle-button`, `checkbox`, `radio`,
//! `switch`, `slider`). Each implements [`Kind`](crate::shim::core::Kind) with a
//! [`KindSpec`](crate::shim::core::KindSpec), runs the `interact` units on skelter events, and records
//! with the `paint` primitives.

pub mod common;
pub mod props;
#[cfg(test)]
pub(crate) mod testkit;

pub mod button;
pub mod checkbox;
pub mod hyperlink;
pub mod icon;
pub mod image;
pub mod label;
pub mod progress;
pub mod radio;
pub mod rich;
pub mod run;
pub mod separator;
pub mod slider;
pub mod spinner;
pub mod switch;
pub mod toggle_button;

pub mod box_;
pub mod frame;
pub mod grid;
pub mod sides;
pub mod stack;

pub use box_::FlexBox;
pub use button::Button;
pub use checkbox::Checkbox;
pub use frame::Frame;
pub use grid::Grid;
pub use hyperlink::Hyperlink;
pub use icon::Icon;
pub use image::Image;
pub use label::Label;
pub use progress::ProgressBar;
pub use radio::RadioButton;
pub use run::{Anchor, Bold, Code, Italic, Span};
pub use separator::Separator;
pub use sides::Sides;
pub use slider::Slider;
pub use spinner::Spinner;
pub use stack::Stack;
pub use switch::Switch;
pub use toggle_button::ToggleButton;
