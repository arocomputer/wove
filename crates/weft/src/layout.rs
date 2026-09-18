//! Bounded integer layouts with fixed and weighted tracks.

/// A rectangle measured in terminal columns and rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    /// Left edge.
    pub x: u16,
    /// Top edge.
    pub y: u16,
    /// Column count.
    pub width: u16,
    /// Row count.
    pub height: u16,
}

impl Rect {
    /// Construct a rectangle; intersections clip overflowing edges.
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Return the shared area of two rectangles.
    pub fn intersection(self, other: Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = (u32::from(self.x) + u32::from(self.width))
            .min(u32::from(other.x) + u32::from(other.width));
        let bottom = (u32::from(self.y) + u32::from(self.height))
            .min(u32::from(other.y) + u32::from(other.height));
        Self::new(
            x,
            y,
            right.saturating_sub(u32::from(x)) as u16,
            bottom.saturating_sub(u32::from(y)) as u16,
        )
    }

    /// Inset all edges, saturating to an empty rectangle when space runs out.
    pub fn inset(self, amount: u16) -> Self {
        let dx = amount.min(self.width / 2);
        let dy = amount.min(self.height / 2);
        Self::new(
            self.x.saturating_add(dx),
            self.y.saturating_add(dy),
            self.width.saturating_sub(amount.saturating_mul(2)),
            self.height.saturating_sub(amount.saturating_mul(2)),
        )
    }
}

/// Direction in which layout tracks are placed.
#[derive(Clone, Copy, Debug)]
pub enum Axis {
    /// Place tracks left to right.
    Horizontal,
    /// Place tracks top to bottom.
    Vertical,
}

/// Space requested by one track.
#[derive(Clone, Copy, Debug)]
pub enum Constraint {
    /// Reserve cells in declaration order, clipped to available space.
    Fixed(u16),
    /// Share remaining space proportionally; zero receives no space.
    Fill(u16),
}

/// A one-dimensional layout; nest layouts to build complex interfaces.
pub struct Layout<'a> {
    /// Placement direction.
    pub axis: Axis,
    /// Track requests.
    pub tracks: &'a [Constraint],
    /// Cells between tracks, clipped when the region is too small.
    pub gap: u16,
}

impl Layout<'_> {
    /// Divide a region without exceeding it; weighted rounding is deterministic.
    pub fn split(&self, area: Rect) -> Vec<Rect> {
        let extent = match self.axis {
            Axis::Horizontal => area.width,
            Axis::Vertical => area.height,
        };
        let gaps = self.tracks.len().saturating_sub(1);
        let gap_total = (gaps as u64 * u64::from(self.gap)).min(u64::from(extent)) as u16;
        let mut remaining = extent - gap_total;
        let mut sizes = vec![0; self.tracks.len()];
        for (i, track) in self.tracks.iter().enumerate() {
            if let Constraint::Fixed(n) = track {
                sizes[i] = (*n).min(remaining);
                remaining -= sizes[i];
            }
        }
        let total: u64 = self
            .tracks
            .iter()
            .map(|c| match c {
                Constraint::Fill(n) => u64::from(*n),
                _ => 0,
            })
            .sum();
        let mut weight = 0u64;
        for (i, track) in self.tracks.iter().enumerate() {
            if let Constraint::Fill(n) = track {
                let before = weight;
                weight += u64::from(*n);
                let end = (u64::from(remaining) * weight)
                    .checked_div(total)
                    .unwrap_or(0);
                let start = (u64::from(remaining) * before)
                    .checked_div(total)
                    .unwrap_or(0);
                sizes[i] = (end - start) as u16;
            }
        }
        let mut offset = 0u16;
        sizes
            .into_iter()
            .map(|size| {
                let size = size.min(extent.saturating_sub(offset));
                let rect = match self.axis {
                    Axis::Horizontal => {
                        Rect::new(area.x.saturating_add(offset), area.y, size, area.height)
                    }
                    Axis::Vertical => {
                        Rect::new(area.x, area.y.saturating_add(offset), area.width, size)
                    }
                };
                offset = offset
                    .saturating_add(size)
                    .saturating_add(self.gap)
                    .min(extent);
                rect
            })
            .collect()
    }
}
