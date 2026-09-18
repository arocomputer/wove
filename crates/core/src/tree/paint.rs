//! Layout measurement and clipped painting of the retained tree.
use super::{Error, Id, Tree};
use crate::{Buffer, Canvas, Rect};
use taffy::{AvailableSpace, Dimension, Display, Size};

impl Tree {
    /// Compute layout and paint only when state or dimensions changed. Callers
    /// borrow the completed frame; repeated reads of an idle tree do no work.
    pub fn frame(&mut self, width: u16, height: u16) -> Result<&Buffer, Error> {
        if self.frame.area().width != width || self.frame.area().height != height {
            self.dirty = true;
        }
        if !self.dirty {
            return Ok(&self.frame);
        }
        if self
            .focus
            .is_some_and(|id| !self.visible(id) || !self.nodes[id].element.focusable())
        {
            self.focus(None)?;
        }
        let root = self.nodes[self.root].layout;
        let mut style = self.layout.style(root)?.clone();
        style.size = Size {
            width: Dimension::length(f32::from(width)),
            height: Dimension::length(f32::from(height)),
        };
        self.layout.set_style(root, style)?;
        let nodes = &self.nodes;
        self.layout.compute_layout_with_measure(
            root,
            Size {
                width: AvailableSpace::Definite(f32::from(width)),
                height: AvailableSpace::Definite(f32::from(height)),
            },
            |inputs, _, context, style| {
                taffy::compute_leaf_layout(
                    inputs,
                    style,
                    |_, _| 0.0,
                    |known, available| {
                        let width = known.width.or(match available.width {
                            AvailableSpace::Definite(w) => Some(w),
                            AvailableSpace::MinContent => Some(1.0),
                            _ => None,
                        });
                        let (w, h) = context.and_then(|id| nodes.get(*id)).map_or((0, 0), |n| {
                            n.element.measure(width.map(|n| n.max(0.0) as u16))
                        });
                        Size {
                            width: known.width.unwrap_or(f32::from(w)),
                            height: known.height.unwrap_or(f32::from(h)),
                        }
                    },
                )
            },
        )?;
        let mut frame = std::mem::replace(&mut self.frame, Buffer::new(0, 0));
        if frame.area().width != width || frame.area().height != height {
            frame = Buffer::new(width, height);
        } else {
            frame.clear();
        }
        self.paint(self.root, (0, 0), frame.area(), &mut frame)?;
        self.frame = frame;
        self.dirty = false;
        Ok(&self.frame)
    }

    fn paint(
        &mut self,
        id: Id,
        parent: (i32, i32),
        clip: Rect,
        buffer: &mut Buffer,
    ) -> Result<(), Error> {
        let node = &mut self.nodes[id];
        if self.layout.style(node.layout)?.display == Display::None {
            return Ok(());
        }
        let layout = *self.layout.layout(node.layout)?;
        let origin = (
            parent.0.saturating_add(layout.location.x as i32),
            parent.1.saturating_add(layout.location.y as i32),
        );
        let size = (
            layout.size.width.max(0.0) as u16,
            layout.size.height.max(0.0) as u16,
        );
        let bounds = clip_signed(origin, size, clip);
        node.origin = origin;
        node.size = size;
        node.clip = bounds;
        node.element.paint(&mut Canvas {
            buffer,
            origin,
            size,
            clip: bounds,
            focused: self.focus == Some(id),
        });
        let inset = (layout.border.left as i32, layout.border.top as i32);
        let inner_size = (
            (layout.size.width - layout.border.left - layout.border.right).max(0.0) as u16,
            (layout.size.height - layout.border.top - layout.border.bottom).max(0.0) as u16,
        );
        let inner = clip_signed((origin.0 + inset.0, origin.1 + inset.1), inner_size, bounds);
        let content = (
            layout.scrollable_overflow_rect.right.max(0.0) as u16,
            layout.scrollable_overflow_rect.bottom.max(0.0) as u16,
        );
        let offset = node.element.viewport(inner_size, content);
        let children = node.children.clone();
        for child in children {
            self.paint(
                child,
                (
                    origin.0 - i32::from(offset.0),
                    origin.1 - i32::from(offset.1),
                ),
                inner,
                buffer,
            )?;
        }
        Ok(())
    }
}

fn clip_signed(origin: (i32, i32), size: (u16, u16), clip: Rect) -> Rect {
    let left = origin.0.max(i32::from(clip.x));
    let top = origin.1.max(i32::from(clip.y));
    let right = (origin.0 + i32::from(size.0)).min(i32::from(clip.x) + i32::from(clip.width));
    let bottom = (origin.1 + i32::from(size.1)).min(i32::from(clip.y) + i32::from(clip.height));
    Rect::new(
        left.clamp(0, i32::from(u16::MAX)) as u16,
        top.clamp(0, i32::from(u16::MAX)) as u16,
        (right - left).max(0) as u16,
        (bottom - top).max(0) as u16,
    )
}
