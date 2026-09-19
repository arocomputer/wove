//! Retained element ownership, layout, focus, and bubbling input.
use crate::{elements::Container, Buffer, Element, Event, Key, Layout, MouseKind, Rect, Response};
mod paint;
use std::any::Any;
use std::{
    collections::HashMap,
    hash::{BuildHasherDefault, Hasher},
    ops::{Index, IndexMut},
    sync::atomic::{AtomicU64, Ordering},
};
use taffy::{Display, TaffyTree};

/// A node identity, unique across trees and never reused.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Id(u64);

/// Ids are sequential integers, so one multiplication spreads them well and
/// costs far less than the default hasher on every node lookup.
#[derive(Default)]
struct IdHasher(u64);
impl Hasher for IdHasher {
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.write_u64(u64::from(*byte) ^ self.0);
        }
    }
    fn write_u64(&mut self, id: u64) {
        self.0 = id.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

#[derive(Default)]
struct Nodes(HashMap<Id, Node, BuildHasherDefault<IdHasher>>);
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
            Self::WrongType => f.write_str("node contains a different element type"),
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
    element: Box<dyn Element>,
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

/// Owns elements and their layout, focus, and last rendered frame.
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
            element: Box::new(Container),
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
    pub fn create(&mut self, element: impl Element) -> Result<Id, Error> {
        let lid = self.layout.new_leaf(element.layout())?;
        let id = self.nodes.insert(Node {
            element: Box::new(element),
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
    pub fn add(&mut self, parent: Id, element: impl Element) -> Result<Id, Error> {
        self.node(parent)?;
        let id = self.create(element)?;
        self.append(parent, id)?;
        Ok(id)
    }
    pub fn append(&mut self, parent: Id, child: Id) -> Result<(), Error> {
        let index = self.node(parent)?.children.len();
        self.insert(parent, child, index)
    }

    /// Move a node without recreating its element, handlers, descendants, or focus.
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
        self.hide(child);
        self.layout.insert_child_at_index(
            self.nodes[parent].layout,
            index,
            self.nodes[child].layout,
        )?;
        self.dirty = true;
        Ok(())
    }

    /// Remove a subtree and drop its element state and callbacks. IDs never revive.
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
    pub fn get<E: Element>(&self, id: Id) -> Result<&E, Error> {
        (self.node(id)?.element.as_ref() as &dyn Any)
            .downcast_ref()
            .ok_or(Error::WrongType)
    }
    /// Mutations invalidate measurement as well as paint; no manual redraw call is needed.
    pub fn update<E: Element>(&mut self, id: Id, update: impl FnOnce(&mut E)) -> Result<(), Error> {
        let node = self.nodes.get_mut(id).ok_or(Error::MissingNode)?;
        let element = (node.element.as_mut() as &mut dyn Any)
            .downcast_mut()
            .ok_or(Error::WrongType)?;
        update(element);
        self.layout.mark_dirty(node.layout)?;
        self.dirty = true;
        Ok(())
    }
    /// A node handler runs before its element's default behavior. Returning handled
    /// stops the default and parent handlers. Replacing it drops the old callback.
    /// Mouse positions are relative to the node's top-left cell.
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
            if !self.visible(id) || !self.nodes[id].element.focusable() {
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
    /// Forget where a subtree was painted, so hit testing cannot reach it. An
    /// empty clip implies empty clips below it, which lets this stop early.
    fn hide(&mut self, id: Id) {
        let node = &mut self.nodes[id];
        let painted = node.clip.width > 0 && node.clip.height > 0;
        node.clip = Rect::default();
        if painted {
            for child in node.children.clone() {
                self.hide(child);
            }
        }
    }
    fn displayed(&self, id: Id) -> bool {
        self.layout
            .style(self.nodes[id].layout)
            .is_ok_and(|s| s.display != Display::None)
    }
    fn visible(&self, id: Id) -> bool {
        let mut next = Some(id);
        while let Some(id) = next {
            let Some(node) = self.nodes.get(id) else {
                return false;
            };
            if !self.displayed(id) {
                return false;
            }
            if id == self.root {
                return true;
            }
            next = node.parent;
        }
        false
    }
    /// Displayed nodes beneath a displayed, attached node, in paint order.
    fn ordered(&self, id: Id, list: &mut Vec<Id>) {
        if !self.displayed(id) {
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
        ids.retain(|id| self.nodes[*id].element.focusable());
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
        // A node sees the pointer relative to its own top-left cell, so it can
        // act on a click without knowing where it was laid out or scrolled to.
        let local;
        let event = match event {
            Event::Mouse(mouse) => {
                let offset = |at: u16, origin: i32| {
                    (i32::from(at) - origin).clamp(0, i32::from(u16::MAX)) as u16
                };
                local = Event::Mouse(crate::Mouse {
                    x: offset(mouse.x, node.origin.0),
                    y: offset(mouse.y, node.origin.1),
                    ..*mouse
                });
                &local
            }
            event => event,
        };
        let mut response = node.handler.as_mut().map_or(Response::IGNORE, |h| h(event));
        if !response.handled {
            let default = node.element.event(event);
            response.handled |= default.handled;
            response.changed |= default.changed;
        }
        if response.changed {
            self.layout.mark_dirty(node.layout)?;
            self.dirty = true;
        }
        Ok(response)
    }
    /// The topmost node painted at a cell. Children are clipped to their
    /// parent, so a subtree that misses the cell is skipped whole.
    fn hit(&self, id: Id, x: u16, y: u16) -> Option<Id> {
        let node = &self.nodes[id];
        if !node.clip.contains(x, y) {
            return None;
        }
        node.children
            .iter()
            .rev()
            .find_map(|child| self.hit(*child, x, y))
            .or(Some(id))
    }
    /// Hit testing uses the last painted frame. Keyboard events target the focus.
    pub fn target(&self, event: &Event) -> Option<Id> {
        if let Event::Mouse(mouse) = event {
            self.hit(self.root, mouse.x, mouse.y)
        } else {
            self.focus
                .filter(|id| self.visible(*id))
                .or(Some(self.root))
        }
    }
    pub fn dispatch(&mut self, event: Event) -> Result<Dispatch, Error> {
        let previous_focus = self.focus;
        if self
            .focus
            .is_some_and(|id| !self.visible(id) || !self.nodes[id].element.focusable())
        {
            self.focus(None)?;
        }
        let target = self.target(&event);
        if matches!(event, Event::Mouse(mouse) if matches!(mouse.kind, MouseKind::Down(_))) {
            let mut ancestor = target;
            while let Some(id) = ancestor {
                if self.nodes[id].element.focusable() {
                    self.focus(Some(id))?;
                    break;
                }
                ancestor = self.nodes[id].parent;
            }
        }
        let mut result = Dispatch {
            target,
            changed: self.focus != previous_focus,
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
}
