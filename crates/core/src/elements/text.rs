//! Plain and styled text use the same grapheme layout for measurement and paint.
use crate::{
    text::{Span, TextLayout, Wrap},
    Canvas, Element, Style,
};

#[derive(Default)]
pub struct Text {
    pub content: String,
    pub style: Style,
    pub wrap: bool,
}
impl Text {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            ..Self::default()
        }
    }
    fn rows(&self, width: Option<u16>) -> TextLayout {
        TextLayout::new(
            &[Span::new(&self.content, self.style)],
            width,
            if self.wrap {
                Wrap::Character
            } else {
                Wrap::None
            },
        )
    }
}
impl Element for Text {
    fn measure(&self, width: Option<u16>) -> (u16, u16) {
        self.rows(width).size()
    }
    fn paint(&self, canvas: &mut Canvas<'_>) {
        self.rows(Some(canvas.size().0)).paint(canvas);
    }
}

/// Styled runs with optional word wrapping. Each span supplies a complete style.
#[derive(Default)]
pub struct RichText {
    pub spans: Vec<Span>,
    pub wrap: Wrap,
}
impl Element for RichText {
    fn measure(&self, width: Option<u16>) -> (u16, u16) {
        TextLayout::new(&self.spans, width, self.wrap).size()
    }
    fn paint(&self, canvas: &mut Canvas<'_>) {
        TextLayout::new(&self.spans, Some(canvas.size().0), self.wrap).paint(canvas);
    }
}
