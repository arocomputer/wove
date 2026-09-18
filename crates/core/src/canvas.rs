//! Widget drawing in local coordinates, clipped by the tree's viewport.
use crate::{Buffer, Rect, Style};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// A widget cannot reach cells outside its assigned clip through this interface.
pub struct Canvas<'a> {
    pub(crate) buffer: &'a mut Buffer,
    pub(crate) origin: (i32, i32),
    pub(crate) clip: Rect,
    pub(crate) size: (u16, u16),
    pub(crate) focused: bool,
}

impl Canvas<'_> {
    pub fn size(&self) -> (u16, u16) {
        self.size
    }
    pub fn focused(&self) -> bool {
        self.focused
    }

    /// Write a single line; partially clipped graphemes are omitted as a whole.
    pub fn text(&mut self, x: i32, y: i32, text: &str, style: Style) {
        let y = self.origin.1.saturating_add(y);
        if y < i32::from(self.clip.y) || y >= i32::from(self.clip.y) + i32::from(self.clip.height) {
            return;
        }
        let mut x = self.origin.0.saturating_add(x);
        let right = i32::from(self.clip.x) + i32::from(self.clip.width);
        for g in text.graphemes(true) {
            if g.chars().any(char::is_control) {
                continue;
            }
            let width = g.width() as i32;
            if width == 0 {
                continue;
            }
            if x >= right {
                break;
            }
            if x >= i32::from(self.clip.x) && x + width <= right {
                self.buffer
                    .write(Rect::new(x as u16, y as u16, width as u16, 1), g, style);
            }
            x = x.saturating_add(width);
        }
    }

    pub fn cursor(&mut self, x: i32, y: i32) {
        let x = self.origin.0.saturating_add(x);
        let y = self.origin.1.saturating_add(y);
        if x >= 0
            && y >= 0
            && x <= i32::from(u16::MAX)
            && y <= i32::from(u16::MAX)
            && self.clip.contains(x as u16, y as u16)
        {
            self.buffer.cursor = Some((x as u16, y as u16));
        }
    }

    pub fn fill(&mut self, style: Style) {
        let row = " ".repeat(usize::from(self.size.0));
        for y in 0..self.size.1 {
            self.text(0, i32::from(y), &row, style);
        }
    }

    pub fn border(&mut self, style: Style) {
        let (w, h) = self.size;
        if w < 2 || h < 2 {
            return;
        }
        let line = "─".repeat(usize::from(w - 2));
        self.text(0, 0, &format!("┌{line}┐"), style);
        self.text(0, i32::from(h - 1), &format!("└{line}┘"), style);
        for y in 1..h - 1 {
            self.text(0, i32::from(y), "│", style);
            self.text(i32::from(w - 1), i32::from(y), "│", style);
        }
    }
}
