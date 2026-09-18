//! Persistent terminal components with layout, input routing, and headless rendering.
//!
//! ```
//! use weft_core::{Tree, widgets::Text};
//! let mut tree = Tree::new();
//! let text = tree.add(tree.root(), Text::new("Hello"))?;
//! tree.update::<Text>(text, |w| w.content.push('!'))?;
//! let frame = tree.frame(80, 24)?;
//! assert_eq!(frame.cell(5, 0).unwrap().symbol(), "!");
//! # Ok::<(), weft_core::Error>(())
//! ```
#![forbid(unsafe_code)]

mod buffer;
mod canvas;
mod event;
mod geometry;
pub mod testing;
pub mod text;
mod tree;
pub mod widgets;

pub use buffer::{Buffer, Cell, Color, Style};
pub use canvas::Canvas;
pub use event::{Event, Key, Modifiers, Mouse, MouseKind, Response};
pub use geometry::Rect;
pub use taffy::Style as Layout;
pub use tree::{Dispatch, Error, Id, Tree, Widget};
/// Taffy's layout types and helpers, measured in terminal cells.
pub mod layout {
    pub use taffy::prelude::*;
}

#[cfg(feature = "terminal")]
pub mod terminal;
