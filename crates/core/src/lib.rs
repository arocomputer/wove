//! Persistent terminal elements with layout, input routing, and headless rendering.
//!
//! ```
//! use wove::{Tree, elements::Text};
//! let mut tree = Tree::new();
//! let text = tree.add(tree.root(), Text::new("Hello"))?;
//! tree.update::<Text>(text, |w| w.content.push('!'))?;
//! let frame = tree.frame(80, 24)?;
//! assert_eq!(frame.cell(5, 0).unwrap().symbol(), "!");
//! # Ok::<(), wove::Error>(())
//! ```
#![forbid(unsafe_code)]

mod buffer;
mod canvas;
pub mod elements;
mod event;
mod geometry;
pub mod testing;
pub mod text;
mod tree;

pub use buffer::{Buffer, Cell, Color, Style};
pub use canvas::Canvas;
pub use event::{Event, Key, Modifiers, Mouse, MouseKind, Response};
pub use geometry::Rect;
pub use taffy::Style as Layout;
pub use tree::{Dispatch, Element, Error, Id, Tree};
/// Taffy's layout types and helpers, measured in terminal cells.
pub mod layout {
    pub use taffy::prelude::*;
}

#[cfg(feature = "terminal")]
pub mod terminal;
