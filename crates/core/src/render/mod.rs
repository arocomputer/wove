//! Cell buffers and clipped drawing independent of terminal output.
mod buffer;
mod canvas;
mod geometry;
pub use buffer::{Buffer, Cell, Color, Style};
pub use canvas::Canvas;
pub use geometry::Rect;
