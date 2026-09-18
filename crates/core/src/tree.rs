//! Retained component ownership, layout, focus, and bubbling input.
use crate::{widgets::Container, Buffer, Canvas, Event, Key, Layout, MouseKind, Rect, Response};
use std::any::Any;
use std::{
    collections::HashMap,
    ops::{Index, IndexMut},
    sync::atomic::{AtomicU64, Ordering},
};
use taffy::{AvailableSpace, Dimension, Display, Size, TaffyTree};

/// A node identity, unique across trees and never reused.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Id(u64);

#[derive(Default)]
struct Nodes(HashMap<Id, Node>);
impl Nodes {
    fn insert(&mut self, node: Node) -> Id {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let id = Id(NEXT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .expect("node identities exhausted"));
        self.0.insert(id, node);
        id
    }
    fn get(&self, id: Id) -> Option<&Node> {
        self.0.get(&id)
    }
    fn get_mut(&mut self, id: Id) -> Option<&mut Node> {
        self.0.get_mut(&id)
    }
    fn remove(&mut self, id: Id) -> Option<Node> {
        self.0.remove(&id)
    }
    fn contains_key(&self, id: Id) -> bool {
        self.0.contains_key(&id)
    }
}
impl Index<Id> for Nodes {
    type Output = Node;
    fn index(&self, id: Id) -> &Node {
        &self.0[&id]
    }
}
impl IndexMut<Id> for Nodes {
    fn index_mut(&mut self, id: Id) -> &mut Node {
        self.0.get_mut(&id).expect("live node")
    }
}

/// A component measures and paints in cells. It owns its state and releases its
/// resources when removed. Events bubble to parents unless consumed.
pub trait Widget: Any {
    fn layout(&self) -> Layout {
        Layout::default()
    }
    fn measure(&self, _width: Option<u16>) -> (u16, u16) {
        (0, 0)
    }
    fn paint(&self, _canvas: &mut Canvas<'_>) {}
    fn event(&mut self, _event: &Event) -> Response {
        Response::IGNORE
    }
    fn focusable(&self) -> bool {
        false
    }
    fn viewport(&mut self, _size: (u16, u16), _content: (u16, u16)) -> (u16, u16) {
        (0, 0)
    }
}

#[derive(Debug)]
pub enum Error {
    MissingNode,
    Root,
    Cycle,
    WrongType,
    Layout(taffy::TaffyError),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingNode => f.write_str("node does not belong to this tree or was removed"),
            Self::Root => f.write_str("the root cannot be removed or reparented"),
            Self::Cycle => f.write_str("reparenting would introduce a cycle"),
            Self::WrongType => f.write_str("node contains a different widget type"),
            Self::Layout(error) => write!(f, "layout: {error}"),
        }
    }
}
impl std::error::Error for Error {}
impl From<taffy::TaffyError> for Error {
    fn from(e: taffy::TaffyError) -> Self {
        Self::Layout(e)
    }
}

type Handler = Box<dyn FnMut(&Event) -> Response>;
struct Node {
    widget: Box<dyn Widget>,
    layout: taffy::NodeId,
    parent: Option<Id>,
    children: Vec<Id>,
    handler: Option<Handler>,
    origin: (i32, i32),
    size: (u16, u16),
    clip: Rect,
}

/// The original target and the nodes visited before an event was consumed.
#[derive(Debug, Default)]
pub struct Dispatch {
    pub target: Option<Id>,
    pub path: Vec<Id>,
    pub handled: bool,
    pub changed: bool,
}

/// Owns widgets and their layout, focus, and last rendered frame.
pub struct Tree {
    nodes: Nodes,
    layout: TaffyTree<Id>,
    root: Id,
    focus: Option<Id>,
    dirty: bool,
    frame: Buffer,
}

impl Default for Tree {
    fn default() -> Self {
        Self::new()
    }
}
impl Tree {
    pub fn new() -> Self {
        let mut nodes = Nodes::default();
        let mut layout = TaffyTree::new();
        let style = Layout {
            flex_direction: taffy::FlexDirection::Column,
            ..Layout::default()
        };
        let lid = layout.new_leaf(style).expect("new root layout");
        let root = nodes.insert(Node {
            widget: Box::new(Container),
            layout: lid,
            parent: None,
            children: vec![],
            handler: None,
            origin: (0, 0),
            size: (0, 0),
            clip: Rect::default(),
        });
        layout
            .set_node_context(lid, Some(root))
            .expect("root exists");
        Self {
            nodes,
            layout,
            root,
            focus: None,
            dirty: true,
            frame: Buffer::new(0, 0),
        }
    }
    pub fn root(&self) -> Id {
        self.root
    }
    pub fn focused(&self) -> Option<Id> {
        self.focus
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    pub fn contains(&self, id: Id) -> bool {
        self.nodes.contains_key(id)
    }
    pub fn parent(&self, id: Id) -> Option<Id> {
        self.nodes.get(id).and_then(|n| n.parent)
    }
    pub fn children(&self, id: Id) -> Result<&[Id], Error> {
        Ok(&self.node(id)?.children)
    }
    /// Visible bounds in the last rendered frame, after ancestor clipping.
    pub fn bounds(&self, id: Id) -> Result<Rect, Error> {
        Ok(self.node(id)?.clip)
    }
    pub fn layout(&self, id: Id) -> Result<&Layout, Error> {
        Ok(self.layout.style(self.node(id)?.layout)?)
    }

    fn node(&self, id: Id) -> Result<&Node, Error> {
        self.nodes.get(id).ok_or(Error::MissingNode)
    }

    /// New nodes are detached. Append or insert them before they can render or focus.
    pub fn create(&mut self, widget: impl Widget) -> Result<Id, Error> {
        let lid = self.layout.new_leaf(widget.layout())?;
        let id = self.nodes.insert(Node {
            widget: Box::new(widget),
            layout: lid,
            parent: None,
            children: vec![],
            handler: None,
            origin: (0, 0),
            size: (0, 0),
            clip: Rect::default(),
        });
        self.layout.set_node_context(lid, Some(id))?;
        Ok(id)
    }
    pub fn add(&mut self, parent: Id, widget: impl Widget) -> Result<Id, Error> {
        self.node(parent)?;
        let id = self.create(widget)?;
        self.append(parent, id)?;
        Ok(id)
    }
    pub fn append(&mut self, parent: Id, child: Id) -> Result<(), Error> {
        let index = self.node(parent)?.children.len();
        self.insert(parent, child, index)
    }

    /// Move a node without recreating its widget, handlers, descendants, or focus.
    /// The index is interpreted after removing it from its previous position.
    pub fn insert(&mut self, parent: Id, child: Id, index: usize) -> Result<(), Error> {
        self.node(parent)?;
        self.node(child)?;
        if child == self.root {
            return Err(Error::Root);
        }
        let mut ancestor = Some(parent);
        while let Some(id) = ancestor {
            if id == child {
                return Err(Error::Cycle);
            }
            ancestor = self.nodes[id].parent;
        }
        if let Some(old) = self.nodes[child].parent {
            self.nodes[old].children.retain(|id| *id != child);
            self.layout
                .remove_child(self.nodes[old].layout, self.nodes[child].layout)?;
        }
        let index = index.min(self.nodes[parent].children.len());
        self.nodes[parent].children.insert(index, child);
        self.nodes[child].parent = Some(parent);
        self.layout.insert_child_at_index(
            self.nodes[parent].layout,
            index,
            self.nodes[child].layout,
        )?;
        self.dirty = true;
        Ok(())
    }

    /// Remove a subtree and drop its widget state and callbacks. IDs never revive.
    pub fn remove(&mut self, id: Id) -> Result<(), Error> {
        self.node(id)?;
        if id == self.root {
            return Err(Error::Root);
        }
        if let Some(parent) = self.nodes[id].parent {
            self.nodes[parent].children.retain(|n| *n != id);
        }
        self.remove_subtree(id)?;
        self.dirty = true;
        Ok(())
    }
    fn remove_subtree(&mut self, id: Id) -> Result<(), Error> {
        if self.focus == Some(id) {
            self.focus(None)?;
        }
        let children = self.nodes[id].children.clone();
        for child in children {
            self.remove_subtree(child)?;
        }
        let node = self.nodes.remove(id).ok_or(Error::MissingNode)?;
        self.layout.remove(node.layout)?;
        Ok(())
    }

    pub fn set_layout(&mut self, id: Id, style: Layout) -> Result<(), Error> {
        self.layout.set_style(self.node(id)?.layout, style)?;
        self.dirty = true;
        Ok(())
    }
    pub fn get<W: Widget>(&self, id: Id) -> Result<&W, Error> {
        (self.node(id)?.widget.as_ref() as &dyn Any)
            .downcast_ref()
            .ok_or(Error::WrongType)
    }
    /// Mutations invalidate measurement as well as paint; no manual redraw call is needed.
    pub fn update<W: Widget>(&mut self, id: Id, update: impl FnOnce(&mut W)) -> Result<(), Error> {
        let node = self.nodes.get_mut(id).ok_or(Error::MissingNode)?;
        let widget = (node.widget.as_mut() as &mut dyn Any)
            .downcast_mut()
            .ok_or(Error::WrongType)?;
        update(widget);
        self.layout.mark_dirty(node.layout)?;
        self.dirty = true;
        Ok(())
    }
    /// A node handler runs before its widget's default behavior. Returning handled
    /// stops the default and parent handlers. Replacing it drops the old callback.
    pub fn on(
        &mut self,
        id: Id,
        handler: impl FnMut(&Event) -> Response + 'static,
    ) -> Result<(), Error> {
        self.nodes.get_mut(id).ok_or(Error::MissingNode)?.handler = Some(Box::new(handler));
        Ok(())
    }
    pub fn focus(&mut self, id: Option<Id>) -> Result<(), Error> {
        if let Some(id) = id {
            self.node(id)?;
            if !self.visible(id) || !self.nodes[id].widget.focusable() {
                return Ok(());
            }
        }
        if self.focus == id {
            return Ok(());
        }
        if let Some(old) = self.focus {
            self.deliver(old, &Event::Blur)?;
        }
        self.focus = id;
        if let Some(id) = id {
            self.deliver(id, &Event::Focus)?;
        }
        self.dirty = true;
        Ok(())
    }
    fn visible(&self, id: Id) -> bool {
        let mut next = Some(id);
        while let Some(id) = next {
            let Some(node) = self.nodes.get(id) else {
                return false;
            };
            if self
                .layout
                .style(node.layout)
                .is_ok_and(|s| s.display == Display::None)
            {
                return false;
            }
            if id == self.root {
                return true;
            }
            next = node.parent;
        }
        false
    }
    fn ordered(&self, id: Id, list: &mut Vec<Id>) {
        if !self.visible(id) {
            return;
        }
        list.push(id);
        for child in &self.nodes[id].children {
            self.ordered(*child, list);
        }
    }
    pub fn focus_next(&mut self, reverse: bool) -> Result<(), Error> {
        let mut ids = Vec::new();
        self.ordered(self.root, &mut ids);
        ids.retain(|id| self.nodes[*id].widget.focusable());
        if ids.is_empty() {
            return self.focus(None);
        }
        let next = match ids.iter().position(|id| Some(*id) == self.focus) {
            Some(i) if reverse => (i + ids.len() - 1) % ids.len(),
            Some(i) => (i + 1) % ids.len(),
            None if reverse => ids.len() - 1,
            None => 0,
        };
        self.focus(Some(ids[next]))
    }
    fn deliver(&mut self, id: Id, event: &Event) -> Result<Response, Error> {
        let node = self.nodes.get_mut(id).ok_or(Error::MissingNode)?;
        let mut response = node.handler.as_mut().map_or(Response::IGNORE, |h| h(event));
        if !response.handled {
            let default = node.widget.event(event);
            response.handled |= default.handled;
            response.changed |= default.changed;
        }
        if response.changed {
            self.layout.mark_dirty(node.layout)?;
            self.dirty = true;
        }
        Ok(response)
    }
    /// Hit testing uses the last painted frame. Keyboard events target the focus.
    pub fn target(&self, event: &Event) -> Option<Id> {
        if let Event::Mouse(mouse) = event {
            let mut ids = Vec::new();
            self.ordered(self.root, &mut ids);
            ids.into_iter()
                .rev()
                .find(|id| self.nodes[*id].clip.contains(mouse.x, mouse.y))
        } else {
            self.focus
                .filter(|id| self.visible(*id))
                .or(Some(self.root))
        }
    }
    pub fn dispatch(&mut self, event: Event) -> Result<Dispatch, Error> {
        if self
            .focus
            .is_some_and(|id| !self.visible(id) || !self.nodes[id].widget.focusable())
        {
            self.focus(None)?;
        }
        let target = self.target(&event);
        if matches!(event, Event::Mouse(mouse) if mouse.kind == MouseKind::Down) {
            let mut ancestor = target;
            while let Some(id) = ancestor {
                if self.nodes[id].widget.focusable() {
                    self.focus(Some(id))?;
                    break;
                }
                ancestor = self.nodes[id].parent;
            }
        }
        let mut result = Dispatch {
            target,
            ..Dispatch::default()
        };
        let mut next = target;
        while let Some(id) = next {
            result.path.push(id);
            let response = self.deliver(id, &event)?;
            result.changed |= response.changed;
            if response.handled {
                result.handled = true;
                break;
            }
            next = self.nodes[id].parent;
        }
        if !result.handled {
            if let Event::Key(Key::Tab, mods) = event {
                self.focus_next(mods.shift)?;
                result.handled = true;
                result.changed = true;
            }
        }
        Ok(result)
    }

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
            .is_some_and(|id| !self.visible(id) || !self.nodes[id].widget.focusable())
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
                            n.widget.measure(width.map(|n| n.max(0.0) as u16))
                        });
                        Size {
                            width: known.width.unwrap_or(f32::from(w)),
                            height: known.height.unwrap_or(f32::from(h)),
                        }
                    },
                )
            },
        )?;
        let mut frame = Buffer::new(width, height);
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
        node.widget.paint(&mut Canvas {
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
        let offset = node.widget.viewport(inner_size, content);
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
