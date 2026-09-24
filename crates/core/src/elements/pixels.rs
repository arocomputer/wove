//! A grid of pixels shown with block characters. Every terminal draws half
//! blocks, which give a cell two pixels, one above the other. Quadrants give
//! four and sextants six; most current terminals draw them, and a few older
//! ones need a font that has them. A cell shows two colors, so each block of
//! pixels is reduced to the pair that loses least, and the glyph whose
//! pattern tells the two apart.
use crate::{Canvas, Color, Element, Style};

/// How many pixels a cell holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Blocks {
    /// One pixel above another: `▀`, `▄`, and `█`.
    #[default]
    Half,
    /// Two by two: `▘`, `▚`, and the rest of the block elements.
    Quadrant,
    /// Two by three, from Unicode's Symbols for Legacy Computing.
    Sextant,
}

impl Blocks {
    /// Pixels per cell, across and down.
    pub fn per_cell(self) -> (usize, usize) {
        match self {
            Self::Half => (1, 2),
            Self::Quadrant => (2, 2),
            Self::Sextant => (2, 3),
        }
    }

    /// The glyph for a pattern of pixels, a bit per pixel in reading order,
    /// for elements that reduce their own pixels to two colors a cell.
    pub fn glyph(self, bits: u8) -> char {
        match self {
            Self::Half => [' ', '▀', '▄', '█'][usize::from(bits & 3)],
            Self::Quadrant => [
                ' ', '▘', '▝', '▀', '▖', '▌', '▞', '▛', '▗', '▚', '▐', '▜', '▄', '▙', '▟', '█',
            ][usize::from(bits & 15)],
            Self::Sextant => match bits & 63 {
                0 => ' ',
                21 => '▌',
                42 => '▐',
                63 => '█',
                bits => {
                    // The block runs through the patterns in order, leaving
                    // out the four that older characters already draw.
                    let skipped = u32::from(bits > 21) + u32::from(bits > 42);
                    char::from_u32(0x1FB00 + u32::from(bits) - 1 - skipped).unwrap_or('█')
                }
            },
        }
    }
}

/// An opaque RGB image, measured in pixels, that paints itself with blocks.
/// Its layout size is the cells its pixels need. Given less, it is clipped;
/// given more, it sits at the top left and the rest of the area is left as
/// the parent painted it.
pub struct Pixels {
    width: usize,
    height: usize,
    data: Vec<[u8; 3]>,
    pub blocks: Blocks,
}

impl Pixels {
    /// A black image.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: vec![[0; 3]; width * height],
            blocks: Blocks::default(),
        }
    }

    pub fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// The cells the image takes: enough for every pixel, rounding up.
    pub fn cells(&self) -> (u16, u16) {
        let (across, down) = self.blocks.per_cell();
        let cells =
            |pixels: usize, per: usize| pixels.div_ceil(per).min(usize::from(u16::MAX)) as u16;
        (cells(self.width, across), cells(self.height, down))
    }

    /// Change the size, keeping the pixels that still fit.
    pub fn resize(&mut self, width: usize, height: usize) {
        if (width, height) == (self.width, self.height) {
            return;
        }
        let mut data = vec![[0; 3]; width * height];
        for y in 0..height.min(self.height) {
            for x in 0..width.min(self.width) {
                data[y * width + x] = self.data[y * self.width + x];
            }
        }
        self.data = data;
        self.width = width;
        self.height = height;
    }

    pub fn fill(&mut self, rgb: [u8; 3]) {
        self.data.fill(rgb);
    }

    /// Set a pixel; one outside the image is ignored.
    pub fn set(&mut self, x: usize, y: usize, rgb: [u8; 3]) {
        if x < self.width && y < self.height {
            self.data[y * self.width + x] = rgb;
        }
    }

    pub fn get(&self, x: usize, y: usize) -> Option<[u8; 3]> {
        (x < self.width && y < self.height).then(|| self.data[y * self.width + x])
    }

    /// Copy from tightly packed RGBA bytes, row by row from the top; alpha is
    /// ignored. Bytes beyond the image are ignored, and missing ones leave
    /// pixels as they were.
    pub fn write(&mut self, rgba: &[u8]) {
        let (bytes, _) = rgba.as_chunks::<4>();
        for (pixel, [r, g, b, _]) in self.data.iter_mut().zip(bytes) {
            *pixel = [*r, *g, *b];
        }
    }
}

impl Element for Pixels {
    fn measure(&self, _width: Option<u16>) -> (u16, u16) {
        self.cells()
    }

    fn paint(&self, canvas: &mut Canvas<'_>) {
        let (across, down) = self.blocks.per_cell();
        let (columns, rows) = self.cells();
        let visible = canvas.visible_rows();
        let first = visible.start.max(0);
        let last = visible.end.min(i32::from(rows));
        let mut block = Vec::with_capacity(across * down);
        for row in first..last {
            for col in 0..usize::from(columns) {
                block.clear();
                for dy in 0..down {
                    for dx in 0..across {
                        let (x, y) = (col * across + dx, row as usize * down + dy);
                        block.push(self.get(x, y).unwrap_or([0; 3]));
                    }
                }
                let (fg, bg, bits) = split(&block);
                let style = Style {
                    fg: color(fg),
                    bg: color(bg),
                    ..Style::default()
                };
                let glyph = self.blocks.glyph(bits);
                canvas.text(col as i32, row, glyph.encode_utf8(&mut [0; 4]), style);
            }
        }
    }
}

fn color([r, g, b]: [u8; 3]) -> Color {
    Color::Rgb(r, g, b)
}

/// The squared distance between two colors.
fn apart(a: [u8; 3], b: [u8; 3]) -> u32 {
    let d = |x: u8, y: u8| (i32::from(x) - i32::from(y)).pow(2) as u32;
    d(a[0], b[0]) + d(a[1], b[1]) + d(a[2], b[2])
}

/// Reduce a block's pixels to two colors: of every pair present, the one
/// that loses least when each pixel takes the nearer. Returns the pair and
/// a bit per pixel that takes the first, in reading order; a block of one
/// color, or none, has no bits set.
pub fn split(block: &[[u8; 3]]) -> ([u8; 3], [u8; 3], u8) {
    if block.is_empty() {
        return ([0; 3], [0; 3], 0);
    }
    let mut colors: Vec<[u8; 3]> = Vec::with_capacity(block.len());
    for &pixel in block {
        if !colors.contains(&pixel) {
            colors.push(pixel);
        }
    }
    let mut best = (colors[0], colors[0], u32::MAX);
    for (i, &fg) in colors.iter().enumerate() {
        for &bg in &colors[i + 1..] {
            let loss: u32 = block
                .iter()
                .map(|&pixel| apart(pixel, fg).min(apart(pixel, bg)))
                .sum();
            if loss < best.2 {
                best = (fg, bg, loss);
            }
        }
    }
    let (fg, bg, _) = best;
    let mut bits = 0;
    for (index, &pixel) in block.iter().enumerate() {
        if fg != bg && apart(pixel, fg) < apart(pixel, bg) {
            bits |= 1 << index;
        }
    }
    (fg, bg, bits)
}
