//! Text storage, editing, and display layout shared by terminal elements.
mod editor;
pub use editor::{Atom, Command, Editor, Motion};
mod input;
pub(crate) use input::edit;
pub use input::{clean, command};
mod layout;
#[doc(hidden)]
pub use layout::Cache;
pub use layout::{wrap, Span, TextLayout, Wrap};
