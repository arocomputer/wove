//! Integer geometry shared by rendering and hit testing.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

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

    pub fn contains(self, x: u16, y: u16) -> bool {
        x >= self.x
            && y >= self.y
            && u32::from(x) < u32::from(self.x) + u32::from(self.width)
            && u32::from(y) < u32::from(self.y) + u32::from(self.height)
    }
}
