//! Text storage, editing, and display layout shared by terminal elements.
mod editor;
pub use editor::Editor;
mod input;
pub(crate) use input::{clean, edit};
mod layout;
pub use layout::{Span, TextLayout, Wrap};
