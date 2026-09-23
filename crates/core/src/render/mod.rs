//! Cell buffers, clipped drawing, and terminal output as plain bytes.
mod buffer;
mod canvas;
mod geometry;
mod inline;
mod pen;
mod renderer;
mod session;
mod surface;
pub(crate) use buffer::{cell_width, clusters};
pub use buffer::{columns, fit, Buffer, Cell, Color, CursorShape, Style, TAB};
pub use canvas::{Border, Canvas};
pub use geometry::Rect;
pub use inline::Inline;
#[cfg(feature = "terminal")]
pub(crate) use inline::Park;
pub use pen::Depth;
pub use renderer::Renderer;
pub use session::{Options, ScreenMode};
pub use surface::{clipboard, progress, title, Progress, BELL};
#[cfg(feature = "terminal")]
pub(crate) use surface::{POP_TITLE, PUSH_TITLE};
