//! Content parsers produce ordinary styled elements without terminal access.
#![forbid(unsafe_code)]
mod code;
mod diff;
mod markdown;
pub use code::{Highlighter, Syntaxes, ThemeSet};
pub use diff::diff;
pub use markdown::{markdown, Palette};
