//! Import as `dioxus_elements` beside `dioxus::prelude::*` to use terminal RSX tags.
#![allow(non_upper_case_globals)]
macro_rules! tags {
    ($($tag:ident),*) => {$(
        pub mod $tag {
            pub const TAG_NAME: &str = stringify!($tag);
            pub const NAME_SPACE: Option<&str> = None;
            pub use super::attrs::*;
        }
    )*};
}
tags!(view, panel, text, input, textarea, scroll);
mod attrs {
    macro_rules! attrs { ($($attr:ident),*) => {$(pub const $attr: (&str, Option<&str>, bool) = (stringify!($attr),None,false);)*}; }
    attrs!(
        layout,
        style,
        width,
        height,
        grow,
        gap,
        padding,
        direction,
        content,
        value,
        placeholder,
        wrap
    );
}
// Dioxus RSX resolves tags through both paths.
#[allow(clippy::module_inception)]
pub mod elements {
    pub use super::{input, panel, scroll, text, textarea, view};
    pub mod completions {
        pub enum CompleteWithBraces {}
    }
}
pub mod events {
    use dioxus_core::{Attribute, AttributeValue, Event};
    macro_rules! events {
        ($(($name:ident,$data:ty)),*) => {$(
            pub fn $name(handler: impl FnMut(Event<$data>) + 'static) -> Attribute {
                Attribute::new(stringify!($name),AttributeValue::listener(handler),None,false)
            }
            pub mod $name {
                pub fn call_with_explicit_closure(handler: impl FnMut(dioxus_core::Event<$data>) + 'static) -> dioxus_core::Attribute { super::$name(handler) }
            }
        )*};
    }
    events!(
        (onkey, wove::Event),
        (onpaste, wove::Event),
        (onmouse, wove::Event),
        (oninput, String),
        (onfocus, wove::Event),
        (onblur, wove::Event),
        (onresize, wove::Event)
    );
}
