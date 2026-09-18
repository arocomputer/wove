//! Differential frame output to any byte sink.
use crate::{Buffer, Color, Style};
use crossterm::{cursor, queue, style};
use std::io::{self, Write};

/// Writes changed cells to any byte sink. Does not acquire terminal modes.
#[derive(Default)]
pub struct Renderer {
    previous: Option<Buffer>,
}

impl Renderer {
    /// Forget physical contents, for example after an external terminal write.
    pub fn invalidate(&mut self) {
        self.previous = None;
    }

    /// Emit changed cells and flush once. Failed writes invalidate the shadow
    /// frame so a subsequent call repaints everything instead of losing changes.
    pub fn draw(&mut self, writer: &mut impl Write, frame: &Buffer) -> io::Result<()> {
        let previous = self.previous.take();
        let previous = previous.as_ref().filter(|old| old.area() == frame.area());
        let mut output = Vec::new();
        let mut next_position = None;
        let mut active_style = None;
        for y in 0..frame.area().height {
            for x in 0..frame.area().width {
                let cell = frame.cell(x, y).expect("frame coordinate");
                if cell.width == 0 {
                    continue;
                }
                if previous.and_then(|old| old.cell(x, y)) == Some(cell) {
                    continue;
                }
                if next_position != Some((x, y)) {
                    queue!(output, cursor::MoveTo(x, y))?;
                }
                if active_style != Some(cell.style) {
                    apply_style(&mut output, cell.style)?;
                    active_style = Some(cell.style);
                }
                output.extend_from_slice(cell.symbol.as_bytes());
                next_position = x.checked_add(cell.width as u16).map(|x| (x, y));
            }
        }
        if !output.is_empty() || previous.is_none_or(|old| old.cursor() != frame.cursor()) {
            match frame.cursor() {
                Some((x, y)) => queue!(output, cursor::MoveTo(x, y), cursor::Show)?,
                None => queue!(output, cursor::Hide)?,
            }
            queue!(
                output,
                style::SetAttribute(style::Attribute::Reset),
                style::ResetColor
            )?;
            writer.write_all(&output)?;
            writer.flush()?;
        }
        self.previous = Some(frame.clone());
        Ok(())
    }
}

/// Convert the portable palette into backend colors.
fn color(value: Color) -> style::Color {
    match value {
        Color::Default => style::Color::Reset,
        Color::Indexed(n) => style::Color::AnsiValue(n),
        Color::Rgb(r, g, b) => style::Color::Rgb { r, g, b },
    }
}

/// Replace all terminal attributes, preventing style leakage between cells.
fn apply_style(writer: &mut impl Write, value: Style) -> io::Result<()> {
    queue!(
        writer,
        style::SetAttribute(style::Attribute::Reset),
        style::SetForegroundColor(color(value.fg)),
        style::SetBackgroundColor(color(value.bg))
    )?;
    if value.bold {
        queue!(writer, style::SetAttribute(style::Attribute::Bold))?;
    }
    if value.italic {
        queue!(writer, style::SetAttribute(style::Attribute::Italic))?;
    }
    if value.reverse {
        queue!(writer, style::SetAttribute(style::Attribute::Reverse))?;
    }
    if value.underline {
        queue!(writer, style::SetAttribute(style::Attribute::Underlined))?;
    }
    Ok(())
}
