//! Cell buffers, clipped drawing, and terminal output as plain bytes.
mod buffer;
mod canvas;
mod geometry;
mod inline;
mod pen;
mod renderer;
mod session;
pub(crate) use buffer::{cluster_width, clusters, graphemes, Slot};
pub use buffer::{Buffer, Cell, Color, Style, TAB};
pub use canvas::{Border, Canvas};
pub use geometry::Rect;
pub use inline::Inline;
pub use pen::Depth;
pub use renderer::Renderer;
pub use session::{Options, ScreenMode};
