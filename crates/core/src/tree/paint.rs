//! Layout measurement and clipped painting of the retained tree.
use super::{Error, Id, Nodes, Tree};
use crate::{elements::Container, Buffer, Canvas, ChildFocus, Children, Rect};
use std::sync::atomic::{AtomicU64, Ordering};
use taffy::{
    AvailableSpace, Dimension, Display, LayoutInput, LayoutOutput, Size, TraversePartialTree,
};

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
        self.drop_stale_focus()?;
        // An inline frame at its natural height was laid out by `height`, and a
        // repaint alone, after a focus change for example, moved nothing.
        let natural = self.fresh == Some((width, None)) && self.natural == Some((width, height));
        if !natural && self.fresh != Some((width, Some(height))) {
            self.compute(width, Some(height))?;
            self.fresh = Some((width, Some(height)));
        }
        // Revealing needs the layout just computed, and precedes the paint.
        if std::mem::take(&mut self.reveal_focus) {
            if let Some(id) = self.focus {
                self.reveal(id)?;
            }
        }
        let mut frame = std::mem::replace(&mut self.frame, Buffer::new(0, 0));
        if frame.area().width != width || frame.area().height != height {
            frame = Buffer::new(width, height);
        } else {
            frame.clear();
        }
        self.paint(self.root, (0, 0), frame.area(), &mut frame)?;
        for index in 0..self.overlays.len() {
            let id = self.overlays[index];
            if self.visible(id) {
                self.paint(id, (0, 0), frame.area(), &mut frame)?;
            } else {
                self.hide(id);
            }
        }
        if let Some((from, to)) = self.selection() {
            frame.invert(from, to);
        }
        // A number no other frame has, so renderers can tell a frame they
        // already drew without comparing its cells.
        static NEXT: AtomicU64 = AtomicU64::new(1);
        frame.version = NEXT.fetch_add(1, Ordering::Relaxed);
        self.frame = frame;
        self.dirty = false;
        Ok(&self.frame)
    }

    /// The height the content wants at a width, for frames that grow with
    /// their content, such as inline sessions. Pass the result to `frame`.
    pub fn height(&mut self, width: u16) -> Result<u16, Error> {
        // An idle tree answers from the last pass, so asking every loop
        // iteration neither lays out nor repaints.
        if let Some((_, height)) = self.natural.filter(|(w, _)| *w == width) {
            return Ok(height);
        }
        self.compute(width, None)?;
        let height = self
            .layout
            .layout(self.nodes[self.root].layout)?
            .size
            .height;
        let height = height.clamp(0.0, f32::from(u16::MAX)) as u16;
        self.fresh = Some((width, None));
        self.natural = Some((width, height));
        self.dirty = true;
        Ok(height)
    }

    /// Lay out the root at a width, and at a height or else its natural one.
    fn compute(&mut self, width: u16, height: Option<u16>) -> Result<(), Error> {
        let root = self.nodes[self.root].layout;
        let size = Size {
            width: Dimension::length(f32::from(width)),
            height: height.map_or(Dimension::auto(), |h| Dimension::length(f32::from(h))),
        };
        // Setting a style invalidates layout, so a repaint at the same size,
        // after a focus change for example, reuses the computed one.
        if self.layout.style(root)?.size != size {
            let mut style = self.layout.style(root)?.clone();
            style.size = size;
            self.layout.set_style(root, style)?;
        }
        let nodes = &self.nodes;
        self.layout.compute_layout_with_measure(
            root,
            Size {
                width: AvailableSpace::Definite(f32::from(width)),
                height: height.map_or(AvailableSpace::MaxContent, |h| {
                    AvailableSpace::Definite(f32::from(h))
                }),
            },
            |inputs, _, context, style| leaf(nodes, inputs, context, style),
        )?;
        Ok(())
    }

    /// Lay out a managed child on its own, as the only child of a column
    /// `width` cells wide, and return the rows it takes with its margins.
    /// Taffy keeps the result until something in the child changes, so asking
    /// again about an unchanged child costs little.
    pub(super) fn lay(&mut self, child: Id, width: u16) -> Result<usize, Error> {
        let (lane, node) = (self.lane, self.nodes[child].layout);
        let columns = Dimension::length(f32::from(width));
        if self.layout.style(lane)?.size.width != columns {
            let mut style = self.layout.style(lane)?.clone();
            style.size.width = columns;
            self.layout.set_style(lane, style)?;
        }
        if self.layout.child_count(lane) != 1 || self.layout.child_at_index(lane, 0)? != node {
            self.layout.set_children(lane, &[node])?;
        }
        let nodes = &self.nodes;
        self.layout.compute_layout_with_measure(
            lane,
            Size {
                width: AvailableSpace::Definite(f32::from(width)),
                height: AvailableSpace::MaxContent,
            },
            |inputs, _, context, style| leaf(nodes, inputs, context, style),
        )?;
        Ok(self.layout.layout(lane)?.size.height.max(0.0) as usize)
    }

    /// Let a custom container measure and place children through the public contract.
    fn paint_managed(
        &mut self,
        id: Id,
        layout: &taffy::Layout,
        origin: (i32, i32),
        clip: Rect,
        buffer: &mut Buffer,
    ) -> Result<(), Error> {
        let area = content(layout, origin);
        // Only last frame's visible children need their hit regions cleared.
        for child in std::mem::take(&mut self.nodes[id].painted) {
            if self.contains(child) {
                self.hide(child);
            }
        }
        let children = std::mem::take(&mut self.nodes[id].children);
        let reveal = self.nodes[id].reveal.take();
        let mut element = std::mem::replace(&mut self.nodes[id].element, Box::new(Container));
        let mut host = ChildHost {
            tree: self,
            children: &children,
            area,
            clip,
            buffer,
            painted: Vec::new(),
            reveal,
        };
        let result = element.arrange(&mut host);
        let painted = host.painted;
        self.nodes[id].children = children;
        self.nodes[id].element = element;
        self.nodes[id].painted = painted;
        result
    }

    fn paint(
        &mut self,
        id: Id,
        parent: (i32, i32),
        clip: Rect,
        buffer: &mut Buffer,
    ) -> Result<(), Error> {
        let node = &self.nodes[id];
        let layout = *self.layout.layout(node.layout)?;
        let place = Placement::new(&layout, parent, clip);
        // Children never draw outside their parent, so a node with nothing
        // visible ends the walk: scrolled-away content costs nothing to paint.
        if self.layout.style(node.layout)?.display == Display::None
            || place.bounds.width == 0
            || place.bounds.height == 0
        {
            self.hide(id);
            return Ok(());
        }
        // The viewport comes first, so an element paints the view it just chose.
        let node = &mut self.nodes[id];
        node.origin = place.origin;
        node.size = place.size;
        node.clip = place.bounds;
        let offset = node.element.viewport(place.inner_size, place.content);
        node.element.paint(&mut Canvas {
            buffer,
            origin: place.origin,
            size: place.size,
            clip: place.bounds,
            focused: self.focus == Some(id),
        });
        let offset = (
            i32::try_from(offset.0).unwrap_or(i32::MAX),
            i32::try_from(offset.1).unwrap_or(i32::MAX),
        );
        if self.managed(id) {
            self.paint_managed(id, &layout, place.origin, place.inner, buffer)?;
        } else {
            let scrolled = (
                place.origin.0.saturating_sub(offset.0),
                place.origin.1.saturating_sub(offset.1),
            );
            for index in 0..self.nodes[id].children.len() {
                let child = self.nodes[id].layers()[index];
                if self.nodes[child].overlay {
                    continue;
                }
                self.paint(child, scrolled, place.inner, buffer)?;
            }
        }
        self.nodes[id].element.overlay(&mut Canvas {
            buffer,
            origin: place.origin,
            size: place.size,
            clip: place.bounds,
            focused: self.focus == Some(id),
        });
        Ok(())
    }
}

/// Narrow access to child layout, with ownership and clipping kept in the tree.
struct ChildHost<'a> {
    tree: &'a mut Tree,
    children: &'a [Id],
    area: (i32, i32, u16, u16),
    clip: Rect,
    buffer: &'a mut Buffer,
    painted: Vec<Id>,
    reveal: Option<(Id, Id)>,
}
impl Children for ChildHost<'_> {
    fn size(&self) -> (u16, u16) {
        (self.area.2, self.area.3)
    }
    fn ids(&self) -> &[Id] {
        self.children
    }
    fn focused(&self) -> Option<usize> {
        let (child, descendant) = self.reveal?;
        if self.tree.focused() != Some(descendant) {
            return None;
        }
        // A request may outlive a clipped frame and a subsequent subtree move.
        let mut at = descendant;
        while at != child {
            at = self.tree.nodes.get(at)?.parent?;
        }
        self.children.iter().position(|id| *id == child)
    }
    fn reveal(&mut self, width: u16) -> Result<Option<ChildFocus>, Error> {
        let Some(index) = self.focused() else {
            return Ok(None);
        };
        self.measure(index, width)?;
        let (child, descendant) = self.reveal.expect("focus request");
        let layout = self
            .tree
            .layout
            .layout(self.tree.nodes[descendant].layout)?;
        let size = (
            layout.size.width.clamp(0.0, f32::from(u16::MAX)) as u16,
            layout.size.height.clamp(0.0, f32::from(u16::MAX)) as u16,
        );
        let (mut x, mut y) = (0.0, 0.0);
        let mut node = descendant;
        loop {
            let location = self
                .tree
                .layout
                .layout(self.tree.nodes[node].layout)?
                .location;
            x += location.x;
            y += location.y;
            if node == child {
                break;
            }
            node = self.tree.nodes[node].parent.ok_or(Error::MissingNode)?;
        }
        Ok(Some(ChildFocus {
            index,
            at: (x.max(0.0) as u32, y.max(0.0) as u32),
            size,
        }))
    }
    fn measure(&mut self, index: usize, width: u16) -> Result<(u32, u32), Error> {
        let child = *self.children.get(index).ok_or(Error::MissingNode)?;
        // Overlays remain attached to the root layout, outside container flow.
        if self.tree.nodes[child].overlay {
            return Ok((0, 0));
        }
        let height = self.tree.lay(child, width)?;
        let layout = self.tree.layout.layout(self.tree.lane)?;
        Ok((
            layout.size.width.max(0.0) as u32,
            height.min(u32::MAX as usize) as u32,
        ))
    }
    fn place(&mut self, index: usize, at: (i32, i32), width: u16) -> Result<(), Error> {
        let child = *self.children.get(index).ok_or(Error::MissingNode)?;
        if self.tree.nodes[child].overlay {
            return Ok(());
        }
        self.measure(index, width)?;
        self.tree.paint(
            child,
            (
                self.area.0.saturating_add(at.0),
                self.area.1.saturating_add(at.1),
            ),
            self.clip,
            self.buffer,
        )?;
        self.painted.push(child);
        Ok(())
    }
}

/// Where a node paints, from its layout and its parent's origin and clip.
struct Placement {
    /// The node's top-left cell in frame coordinates.
    origin: (i32, i32),
    /// The node's size as cells; a taller container saturates.
    size: (u16, u16),
    /// The node's visible cells.
    bounds: Rect,
    /// The visible cells inside its border, which clip its children.
    inner: Rect,
    inner_size: (u16, u16),
    /// The extent of its children's content, for its viewport.
    content: (u32, u32),
}

impl Placement {
    fn new(layout: &taffy::Layout, parent: (i32, i32), clip: Rect) -> Self {
        let cells = |n: u32| n.min(u32::from(u16::MAX)) as u16;
        let origin = (
            parent.0.saturating_add(layout.location.x as i32),
            parent.1.saturating_add(layout.location.y as i32),
        );
        // A container may be taller than a cell coordinate can express; its
        // clip uses the full extent even though its own canvas saturates.
        let extent = (
            layout.size.width.max(0.0) as u32,
            layout.size.height.max(0.0) as u32,
        );
        let bounds = clip_signed(origin, extent, clip);
        let inner_extent = (
            (layout.size.width - layout.border.left - layout.border.right).max(0.0) as u32,
            (layout.size.height - layout.border.top - layout.border.bottom).max(0.0) as u32,
        );
        let inset = (
            origin.0.saturating_add(layout.border.left as i32),
            origin.1.saturating_add(layout.border.top as i32),
        );
        Self {
            origin,
            size: (cells(extent.0), cells(extent.1)),
            bounds,
            inner: clip_signed(inset, inner_extent, bounds),
            inner_size: (cells(inner_extent.0), cells(inner_extent.1)),
            content: (
                layout.scrollable_overflow_rect.right.max(0.0) as u32,
                layout.scrollable_overflow_rect.bottom.max(0.0) as u32,
            ),
        }
    }
}

fn clip_signed(origin: (i32, i32), size: (u32, u32), clip: Rect) -> Rect {
    let left = i64::from(origin.0).max(i64::from(clip.x));
    let top = i64::from(origin.1).max(i64::from(clip.y));
    let right =
        (i64::from(origin.0) + i64::from(size.0)).min(i64::from(clip.x) + i64::from(clip.width));
    let bottom =
        (i64::from(origin.1) + i64::from(size.1)).min(i64::from(clip.y) + i64::from(clip.height));
    // The clip lies within the frame, so clamped edges fit a cell coordinate.
    Rect::new(
        left.clamp(0, i64::from(u16::MAX)) as u16,
        top.clamp(0, i64::from(u16::MAX)) as u16,
        (right - left).clamp(0, i64::from(u16::MAX)) as u16,
        (bottom - top).clamp(0, i64::from(u16::MAX)) as u16,
    )
}

/// Lay out a leaf by measuring its element, which has cells, not pixels.
fn leaf(
    nodes: &Nodes,
    inputs: LayoutInput,
    context: Option<&mut Id>,
    style: &taffy::Style,
) -> LayoutOutput {
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
}

/// A node's content box, inside its border and padding: where it is, in
/// frame coordinates from its `origin`, and its size.
pub(super) fn content(layout: &taffy::Layout, origin: (i32, i32)) -> (i32, i32, u16, u16) {
    let cells = |n: f32| n.clamp(0.0, f32::from(u16::MAX)) as u16;
    (
        origin
            .0
            .saturating_add((layout.border.left + layout.padding.left) as i32),
        origin
            .1
            .saturating_add((layout.border.top + layout.padding.top) as i32),
        cells(layout.content_box_width()),
        cells(layout.content_box_height()),
    )
}
