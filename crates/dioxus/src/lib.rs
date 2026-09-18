//! Dioxus components rendered by Wove's element tree.
#![forbid(unsafe_code)]
pub mod elements;
mod host;
mod view;
pub use dioxus_core::AttributeValue;
pub use host::{Error, Registry};
pub use view::View;
#[cfg(feature = "terminal")]
mod runtime;
#[cfg(feature = "terminal")]
pub use runtime::run;
