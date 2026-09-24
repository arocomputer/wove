//! Piece designs as geometry, drawn at whatever size a square has. Each
//! piece is a union of circles, ellipses, and polygons in a unit square,
//! minus a few cuts; rasterizing it gives a mask, and the mask gives an
//! outline where it meets the outside and a highlight and a shade where
//! light from the upper left would fall. Nothing is stored as a bitmap, so
//! there is no size at which the pieces are stretched.
use crate::rules::Kind;

/// What a pixel of a drawn piece is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// The square shows through.
    None,
    Outline,
    Fill,
    Highlight,
    Shade,
}

/// A shape in a unit square with `y` running down.
enum Shape {
    Circle(f32, f32, f32),
    Ellipse(f32, f32, f32, f32),
    Rect(f32, f32, f32, f32),
    Poly(&'static [(f32, f32)]),
}

impl Shape {
    fn contains(&self, x: f32, y: f32) -> bool {
        match *self {
            Self::Circle(cx, cy, r) => (x - cx).powi(2) + (y - cy).powi(2) <= r * r,
            Self::Ellipse(cx, cy, rx, ry) => {
                ((x - cx) / rx).powi(2) + ((y - cy) / ry).powi(2) <= 1.0
            }
            Self::Rect(x0, y0, x1, y1) => x >= x0 && x <= x1 && y >= y0 && y <= y1,
            Self::Poly(points) => {
                // Even-odd rule: count the edges a ray to the right crosses.
                let mut inside = false;
                let mut j = points.len() - 1;
                for i in 0..points.len() {
                    let (xi, yi) = points[i];
                    let (xj, yj) = points[j];
                    if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
                        inside = !inside;
                    }
                    j = i;
                }
                inside
            }
        }
    }
}

struct Design {
    parts: &'static [Shape],
    /// Small features, added and cut only once a piece is big enough for
    /// them to read as anything but specks.
    details: &'static [Shape],
    cuts: &'static [Shape],
    /// Lines drawn in the outline color on the piece itself, such as a
    /// rook's notches: they show without breaking the silhouette.
    marks: &'static [Shape],
}

/// The size from which details are drawn, and the one from which marks are.
const DETAIL: usize = 24;
const MARKS: usize = 12;

const BASE: Shape = Shape::Rect(0.15, 0.80, 0.85, 0.90);
const COLLAR: Shape = Shape::Ellipse(0.5, 0.52, 0.25, 0.05);
const BODY: Shape = Shape::Poly(&[(0.35, 0.55), (0.65, 0.55), (0.74, 0.80), (0.26, 0.80)]);

const PAWN: Design = Design {
    parts: &[
        Shape::Circle(0.5, 0.25, 0.16),
        Shape::Poly(&[(0.41, 0.38), (0.59, 0.38), (0.63, 0.52), (0.37, 0.52)]),
        Shape::Ellipse(0.5, 0.52, 0.22, 0.05),
        Shape::Poly(&[(0.40, 0.55), (0.60, 0.55), (0.72, 0.80), (0.28, 0.80)]),
        BASE,
    ],
    details: &[],
    cuts: &[],
    marks: &[],
};

const KNIGHT: Design = Design {
    parts: &[
        Shape::Poly(&[
            (0.12, 0.40),
            (0.20, 0.28),
            (0.30, 0.20),
            (0.36, 0.06),
            (0.44, 0.18),
            (0.54, 0.06),
            (0.62, 0.20),
            (0.72, 0.30),
            (0.82, 0.50),
            (0.84, 0.80),
            (0.26, 0.80),
            (0.34, 0.68),
            (0.42, 0.58),
            (0.38, 0.52),
            (0.20, 0.54),
            (0.12, 0.50),
        ]),
        BASE,
    ],
    details: &[],
    cuts: &[Shape::Circle(0.40, 0.31, 0.035)],
    marks: &[],
};

const BISHOP: Design = Design {
    parts: &[
        Shape::Poly(&[
            (0.50, 0.06),
            (0.68, 0.32),
            (0.68, 0.48),
            (0.32, 0.48),
            (0.32, 0.32),
        ]),
        Shape::Ellipse(0.5, 0.50, 0.22, 0.05),
        Shape::Poly(&[(0.40, 0.53), (0.60, 0.53), (0.72, 0.80), (0.28, 0.80)]),
        BASE,
    ],
    details: &[Shape::Circle(0.5, 0.06, 0.05)],
    cuts: &[Shape::Poly(&[
        (0.56, 0.17),
        (0.63, 0.22),
        (0.47, 0.44),
        (0.40, 0.39),
    ])],
    marks: &[],
};

const ROOK: Design = Design {
    parts: &[
        Shape::Rect(0.175, 0.10, 0.825, 0.36),
        Shape::Rect(0.30, 0.36, 0.70, 0.72),
        Shape::Rect(0.20, 0.72, 0.80, 0.80),
        BASE,
    ],
    details: &[],
    cuts: &[],
    marks: &[
        Shape::Rect(0.31, 0.10, 0.44, 0.26),
        Shape::Rect(0.56, 0.10, 0.69, 0.26),
    ],
};

const QUEEN: Design = Design {
    parts: &[
        Shape::Circle(0.28, 0.20, 0.10),
        Shape::Circle(0.50, 0.14, 0.11),
        Shape::Circle(0.72, 0.20, 0.10),
        Shape::Poly(&[(0.20, 0.22), (0.80, 0.22), (0.72, 0.52), (0.28, 0.52)]),
        COLLAR,
        BODY,
        BASE,
    ],
    details: &[],
    cuts: &[],
    marks: &[],
};

const KING: Design = Design {
    parts: &[
        Shape::Rect(0.43, 0.02, 0.57, 0.30),
        Shape::Rect(0.28, 0.10, 0.72, 0.21),
        Shape::Poly(&[(0.22, 0.30), (0.78, 0.30), (0.72, 0.52), (0.28, 0.52)]),
        COLLAR,
        BODY,
        BASE,
    ],
    details: &[],
    cuts: &[],
    marks: &[],
};

fn design(kind: Kind) -> &'static Design {
    match kind {
        Kind::Pawn => &PAWN,
        Kind::Knight => &KNIGHT,
        Kind::Bishop => &BISHOP,
        Kind::Rook => &ROOK,
        Kind::Queen => &QUEEN,
        Kind::King => &KING,
    }
}

/// Draw a piece into `width` by `height` pixels, row by row. The square is
/// taken to be square on screen whatever its pixels' proportions, so shapes
/// keep theirs. Edges are settled by sampling each pixel nine times.
pub fn raster(kind: Kind, width: usize, height: usize) -> Vec<Tone> {
    let design = design(kind);
    let detailed = width.min(height) >= DETAIL;
    let inside = |x: f32, y: f32| {
        let part = design.parts.iter().any(|part| part.contains(x, y));
        let detail = detailed && design.details.iter().any(|part| part.contains(x, y));
        let cut = detailed && design.cuts.iter().any(|cut| cut.contains(x, y));
        (part || detail) && !cut
    };
    let mut mask = vec![false; width * height];
    for py in 0..height {
        for px in 0..width {
            let mut hits = 0;
            for sy in 0..3 {
                for sx in 0..3 {
                    let x = (px as f32 + (sx as f32 + 0.5) / 3.0) / width as f32;
                    let y = (py as f32 + (sy as f32 + 0.5) / 3.0) / height as f32;
                    hits += u8::from(inside(x, y));
                }
            }
            mask[py * width + px] = hits >= 5;
        }
    }
    let at = |x: isize, y: isize| {
        x >= 0
            && y >= 0
            && (x as usize) < width
            && (y as usize) < height
            && mask[y as usize * width + x as usize]
    };
    // The outline is the pixels of the mask that touch the outside, grown
    // thicker as squares grow. Inside it, the band along the upper left
    // catches the light and the band along the lower right falls into
    // shade, once a piece is big enough for bands to read as form.
    let thick = (width.min(height) / 24).max(1) as isize;
    let edge = |x: isize, y: isize, dx: isize, dy: isize| {
        (1..=thick).any(|step| !at(x + dx * step, y + dy * step))
    };
    let shaded = width.min(height) >= 20;
    // A tiny piece cannot afford a line all around it; a shadow along its
    // lower right keeps it off the square without eating its shape.
    let tiny = width.min(height) < 14;
    let mut tones = vec![Tone::None; width * height];
    for py in 0..height as isize {
        for px in 0..width as isize {
            if !at(px, py) {
                continue;
            }
            let outline = edge(px, py, 1, 0)
                || edge(px, py, 0, 1)
                || (!tiny && (edge(px, py, -1, 0) || edge(px, py, 0, -1)));
            let lit = edge(px - thick, py, -1, 0) || edge(px, py - thick, 0, -1);
            let dark = edge(px + thick, py, 1, 0) || edge(px, py + thick, 0, 1);
            let index = py as usize * width + px as usize;
            let marked = width.min(height) >= MARKS && {
                let (x, y) = (
                    (px as f32 + 0.5) / width as f32,
                    (py as f32 + 0.5) / height as f32,
                );
                design.marks.iter().any(|mark| mark.contains(x, y))
            };
            tones[index] = match (outline || marked, shaded && lit, shaded && dark) {
                (true, _, _) => Tone::Outline,
                (_, true, _) => Tone::Highlight,
                (_, _, true) => Tone::Shade,
                _ => Tone::Fill,
            };
        }
    }
    tones
}
