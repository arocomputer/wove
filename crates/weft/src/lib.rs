//! weft composes terminal interfaces from application-owned state and widgets.
//!
//! Render into a [`Buffer`] for tests or use [`Terminal`] with the default
//! `terminal` feature. Drawing and layout need no terminal or async runtime.
//!
//! ```
//! use weft::{Buffer, Style, Text, Widget};
//! let mut frame = Buffer::new(20, 1);
//! Text { content: "Hello, terminal.", style: Style::default() }
//!     .render(frame.area(), &mut frame);
//! assert_eq!(frame.lines()[0].trim_end(), "Hello, terminal.");
//! ```
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod buffer;
mod layout;
mod widgets;

pub use buffer::{Buffer, Cell, Color, Style};
pub use layout::{Axis, Constraint, Layout, Rect};
pub use widgets::{Border, List, Text, Widget};

#[cfg(feature = "terminal")]
mod terminal;
#[cfg(feature = "terminal")]
pub use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
#[cfg(feature = "terminal")]
pub use terminal::{run, Application, Renderer, Terminal};
