//! Retained element ownership, layout, focus, and bubbling input.
use crate::{elements::Container, Buffer, Element, Event, Key, Layout, MouseKind, Rect, Response};
mod layers;
mod paint;
use std::any::Any;
use std::{
    collections::HashMap,
    hash::{BuildHasherDefault, Hasher},
    ops::{Index, IndexMut},
    sync::atomic::{AtomicU64, Ordering},
};
use taffy::{Display, Overflow, TaffyTree};

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
    /// Paint order among siblings; higher paints later and is hit first.
    z: i16,
    /// The children sorted by `z`, kept only while one of them has a `z`.
    /// Empty means child order is paint order.
    order: Vec<Id>,
    /// Managed children painted last frame, in paint order.
    painted: Vec<Id>,
    overlay: bool,
    pointer_events: bool,
    /// The managed child and its descendant to reveal at the next arrangement.
    reveal: Option<(Id, Id)>,
}
impl Node {
    /// A detached node with no children or handler.
    fn new(element: Box<dyn Element>, layout: taffy::NodeId) -> Self {
        Self {
            element,
            layout,
            parent: None,
            children: vec![],
            handler: None,
            origin: (0, 0),
            size: (0, 0),
            clip: Rect::default(),
            z: 0,
            order: vec![],
            painted: vec![],
            overlay: false,
            pointer_events: true,
            reveal: None,
        }
    }
    /// The children in paint order.
    fn layers(&self) -> &[Id] {
        if self.order.is_empty() {
            &self.children
        } else {
            &self.order
        }
    }
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
    /// The arguments of the layout pass whose results are still valid. Any
    /// change that can move a node clears it; repaints alone keep it.
    fresh: Option<(u16, Option<u16>)>,
    /// The content's natural height at a width, while `fresh`.
    natural: Option<(u16, u16)>,
    /// The node that took the last mouse press. It receives the drag and the
    /// release wherever the pointer goes.
    capture: Option<Id>,
    /// The node under the pointer, for enter and leave events.
    hover: Option<Id>,
    selectable: bool,
    /// Where a press that no node wanted began, until the button is released.
    anchor: Option<(u16, u16)>,
    /// A selection of the painted screen, as two cells in frame coordinates.
    selection: Option<((u16, u16), (u16, u16))>,
    /// The focus moved and the next frame should scroll it into view.
    reveal_focus: bool,
    /// A column that lays out one managed child at a time, apart from
    /// the rest of the layout, which never includes those children.
    lane: taffy::NodeId,
    frame: Buffer,
    overlays: Vec<Id>,
    scopes: Vec<(Id, Option<Id>)>,
    /// Logical selection remains attached to content across scrolling and reflow.
    text_selection: Option<Id>,
    text_anchor: Option<usize>,
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
        let lid = layout.new_leaf(style.clone()).expect("new root layout");
        let root = nodes.insert(Node::new(Box::new(Container), lid));
        layout
            .set_node_context(lid, Some(root))
            .expect("root exists");
        let lane = layout.new_leaf(style).expect("new lane layout");
        Self {
            nodes,
            layout,
            root,
            focus: None,
            dirty: true,
            fresh: None,
            natural: None,
            capture: None,
            hover: None,
            selectable: true,
            anchor: None,
            selection: None,
            reveal_focus: false,
            lane,
            frame: Buffer::new(0, 0),
            overlays: Vec::new(),
            scopes: Vec::new(),
            text_selection: None,
            text_anchor: None,
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
    /// Invalidate layout as well as paint.
    fn touch(&mut self) {
        self.dirty = true;
        self.fresh = None;
        self.natural = None;
    }

    /// Order a node among its siblings. Children paint in ascending `z`, then
    /// in child order, and the pointer hits them in reverse. Use it with
    /// absolute positioning for menus, dialogs, and other overlays.
    pub fn set_z(&mut self, id: Id, z: i16) -> Result<(), Error> {
        self.nodes.get_mut(id).ok_or(Error::MissingNode)?.z = z;
        if let Some(parent) = self.nodes[id].parent {
            self.restack(parent, id);
        }
        self.dirty = true;
        self.sort_overlays();
        Ok(())
    }
    /// Sort a node's children into paint order after `child` joins, leaves,
    /// or changes its `z`, so painting and hit testing never sort. Children
    /// without a `z` cost nothing, which keeps appending to a long list linear.
    fn restack(&mut self, id: Id, child: Id) {
        let node = &self.nodes[id];
        if node.order.is_empty() && self.nodes[child].z == 0 {
            return;
        }
        let mut order = Vec::new();
        if node.children.iter().any(|child| self.nodes[*child].z != 0) {
            order.clone_from(&node.children);
            order.sort_by_key(|child| self.nodes[*child].z);
        }
        self.nodes[id].order = order;
    }

    /// Whether a drag that no node handles selects text on the screen.
    /// On by default, because capturing the mouse takes the terminal's own
    /// selection away from the user.
    pub fn set_selectable(&mut self, selectable: bool) {
        self.selectable = selectable;
        self.clear_selection();
    }
    /// The selected cells, first and last in reading order.
    pub fn selection(&self) -> Option<((u16, u16), (u16, u16))> {
        self.selection.map(|(a, b)| {
            if (a.1, a.0) <= (b.1, b.0) {
                (a, b)
            } else {
                (b, a)
            }
        })
    }
    pub fn clear_selection(&mut self) {
        self.anchor = None;
        self.text_anchor = None;
        if let Some(id) = self.text_selection.take() {
            if let Some(node) = self.nodes.get_mut(id) {
                node.element.select(None);
            }
            self.dirty = true;
        }
        if self.selection.take().is_some() {
            self.dirty = true;
        }
    }
    /// Logical selected text when available, otherwise the last painted screen
    /// selection with one line per row, for the clipboard.
    pub fn selected_text(&self) -> Option<String> {
        if let Some(id) = self.text_selection {
            return self.nodes.get(id)?.element.selected_text();
        }
        if let Some((from, to)) = self.selection() {
            return Some(self.frame.text(from, to));
        }
        None
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
        let id = self.nodes.insert(Node::new(Box::new(element), lid));
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
            self.nodes[old].painted.retain(|id| *id != child);
            self.restack(old, child);
        }
        // A managed child has no parent in the layout, or the lane.
        let node = self.nodes[child].layout;
        if let Some(old) = self.layout.parent(node) {
            self.layout.remove_child(old, node)?;
        }
        let index = index.min(self.nodes[parent].children.len());
        self.nodes[parent].children.insert(index, child);
        self.nodes[child].parent = Some(parent);
        self.restack(parent, child);
        self.hide(child);
        if self.nodes[child].overlay {
            self.layout.add_child(self.nodes[self.root].layout, node)?;
        } else if !self.managed(parent) {
            if index + 1 == self.nodes[parent].children.len() {
                self.layout.add_child(self.nodes[parent].layout, node)?;
            } else {
                let index = self.nodes[parent].children[..index]
                    .iter()
                    .filter(|id| !self.nodes[**id].overlay)
                    .count();
                self.layout
                    .insert_child_at_index(self.nodes[parent].layout, index, node)?;
            }
        }
        self.touch();
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
            self.nodes[parent].painted.retain(|n| *n != id);
            self.restack(parent, id);
        }
        self.remove_subtree(id)?;
        self.restore_scopes()?;
        self.touch();
        Ok(())
    }
    fn remove_subtree(&mut self, id: Id) -> Result<(), Error> {
        if self.focus == Some(id) {
            self.focus(None)?;
        }
        for child in std::mem::take(&mut self.nodes[id].children) {
            self.remove_subtree(child)?;
        }
        let node = self.nodes.remove(id).ok_or(Error::MissingNode)?;
        self.overlays.retain(|overlay| *overlay != id);
        self.layout.remove(node.layout)?;
        Ok(())
    }

    pub fn set_layout(&mut self, id: Id, mut style: Layout) -> Result<(), Error> {
        if self.node(id)?.overlay {
            style.position = taffy::Position::Absolute;
        }
        self.layout.set_style(self.node(id)?.layout, style)?;
        self.touch();
        self.drop_stale_focus()?;
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
        self.touch();
        Ok(())
    }
    /// Mutate appearance without invalidating measurement. The caller promises
    /// that intrinsic size and layout are unchanged; use `update` otherwise.
    pub fn repaint<E: Element>(
        &mut self,
        id: Id,
        update: impl FnOnce(&mut E),
    ) -> Result<(), Error> {
        let node = self.nodes.get_mut(id).ok_or(Error::MissingNode)?;
        let element = (node.element.as_mut() as &mut dyn Any)
            .downcast_mut()
            .ok_or(Error::WrongType)?;
        update(element);
        self.dirty = true;
        Ok(())
    }
    /// A node handler runs before its element's default behavior. Returning handled
    /// stops the default and parent handlers. Replacing it drops the old callback.
    /// Mouse positions are relative to the node's top-left cell, and never
    /// negative: a captured drag past the node's left or top edge reports zero.
    pub fn on(
        &mut self,
        id: Id,
        handler: impl FnMut(&Event) -> Response + 'static,
    ) -> Result<(), Error> {
        self.nodes.get_mut(id).ok_or(Error::MissingNode)?.handler = Some(Box::new(handler));
        Ok(())
    }
    /// Focus a displayed, focusable node, or clear the focus. The next frame
    /// asks the nearest scrolling or managed ancestor to show the node.
    pub fn focus(&mut self, id: Option<Id>) -> Result<(), Error> {
        if let Some(id) = id {
            self.node(id)?;
            if !self.visible(id) || !self.in_scope(id) || !self.nodes[id].element.focusable() {
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
        // Focus changes how nodes paint, not where they are.
        self.dirty = true;
        self.reveal_focus = id.is_some();
        Ok(())
    }
    /// Ask the nearest scrolling or managed ancestor
    /// to show a node, with the node's place in that ancestor's content.
    /// Needs a current layout.
    fn reveal(&mut self, id: Id) -> Result<(), Error> {
        // The ancestor, and its child that holds the node.
        let mut child = id;
        let ancestor = loop {
            if self.nodes[child].overlay {
                return Ok(());
            }
            let Some(parent) = self.nodes[child].parent else {
                return Ok(());
            };
            let overflow = self.layout.style(self.nodes[parent].layout)?.overflow;
            if self.managed(parent)
                || overflow.x == Overflow::Scroll
                || overflow.y == Overflow::Scroll
            {
                break parent;
            }
            child = parent;
        };
        if self.managed(ancestor) {
            self.nodes[ancestor].reveal = Some((child, id));
            return Ok(());
        }
        let layout = self.layout.layout(self.nodes[id].layout)?;
        let size = (
            layout.size.width.clamp(0.0, f32::from(u16::MAX)) as u16,
            layout.size.height.clamp(0.0, f32::from(u16::MAX)) as u16,
        );
        let (mut x, mut y) = (0.0, 0.0);
        let mut node = id;
        loop {
            let location = self.layout.layout(self.nodes[node].layout)?.location;
            x += location.x;
            y += location.y;
            match self.nodes[node].parent {
                Some(parent) if node != child => node = parent,
                _ => break,
            }
        }
        let at = (x.max(0.0) as u32, y.max(0.0) as u32);
        self.nodes[ancestor].element.reveal(at, size);
        Ok(())
    }
    /// Whether an element owns child placement instead of using flex/grid.
    fn managed(&self, id: Id) -> bool {
        self.nodes[id].element.manages_children()
    }
    /// Forget where a subtree was painted, so hit testing cannot reach it. An
    /// empty clip implies empty clips below it, which lets this stop early.
    fn hide(&mut self, id: Id) {
        let node = &mut self.nodes[id];
        let painted = node.clip.width > 0 && node.clip.height > 0;
        node.clip = Rect::default();
        if painted {
            let children = if node.element.manages_children() {
                node.painted.clone()
            } else {
                node.children.clone()
            };
            for child in children {
                if !self.contains(child) {
                    continue;
                }
                if self.nodes[child].overlay {
                    continue;
                }
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
    /// Displayed nodes beneath a displayed, attached node, depth first in
    /// child order. This is the focus order; `z` changes painting, not focus.
    fn ordered(&self, id: Id, list: &mut Vec<Id>) {
        if !self.displayed(id) {
            return;
        }
        list.push(id);
        for child in &self.nodes[id].children {
            self.ordered(*child, list);
        }
    }
    /// Focus the next focusable node in child order, depth first, or the
    /// previous one with `reverse`, wrapping at either end.
    pub fn focus_next(&mut self, reverse: bool) -> Result<(), Error> {
        let mut ids = Vec::new();
        self.ordered(self.scope().unwrap_or(self.root), &mut ids);
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
            response.layout |= default.layout;
        }
        response.changed |= response.layout;
        if response.layout {
            self.layout.mark_dirty(node.layout)?;
            self.touch();
        } else if response.changed {
            self.dirty = true;
        }
        Ok(response)
    }
    /// The topmost node painted at a cell. Children are clipped to their
    /// parent, so a subtree that misses the cell is skipped whole. Managed
    /// containers hit only the children placed in their last frame.
    fn hit(&self, id: Id, x: u16, y: u16) -> Option<Id> {
        let node = &self.nodes[id];
        if !node.pointer_events || !node.clip.contains(x, y) {
            return None;
        }
        let children = if self.managed(id) {
            &node.painted[..]
        } else {
            node.layers()
        };
        children
            .iter()
            .rev()
            .filter(|child| !self.nodes[**child].overlay)
            .find_map(|child| self.hit(*child, x, y))
            .or(Some(id))
    }
    /// Resolve the same target used by dispatch: captured drags and releases go
    /// to the press owner, other mouse events hit the last painted frame, and
    /// keyboard events target the focus. Adapters can route listeners first.
    pub fn target(&self, event: &Event) -> Option<Id> {
        if let Event::Mouse(mouse) = event {
            if matches!(mouse.kind, MouseKind::Drag(_) | MouseKind::Up(_)) {
                if let Some(id) = self
                    .capture
                    .filter(|id| self.visible(*id) && self.in_scope(*id))
                {
                    return Some(id);
                }
            }
            self.overlays
                .iter()
                .rev()
                .filter(|id| {
                    self.visible(**id) && self.in_scope(**id) && self.pointer_enabled(**id)
                })
                .find_map(|id| self.hit(*id, mouse.x, mouse.y))
                .or_else(|| self.hit(self.scope().unwrap_or(self.root), mouse.x, mouse.y))
                .or(self.scope())
        } else {
            self.focus
                .filter(|id| self.visible(*id) && self.in_scope(*id))
                .or(Some(self.scope().unwrap_or(self.root)))
        }
    }

    /// End pointer capture and any pending screen-selection drag. An adapter
    /// that prevents a press or release must still end the previous gesture.
    /// The visible selection is kept until cleared or replaced by a new press.
    pub fn release_pointer(&mut self) {
        self.capture = None;
        self.anchor = None;
        self.text_anchor = None;
    }

    /// What becomes of a mouse event before any node sees it: selection and
    /// capture are the tree's business, not an element's.
    fn pointer(&mut self, mouse: &crate::Mouse, target: Option<Id>) -> Result<Pointer, Error> {
        let mut changed = false;
        if let (Some(id), Some(anchor)) = (self.text_selection, self.text_anchor) {
            if matches!(mouse.kind, MouseKind::Drag(_)) {
                if let Some(at) = self.logical_position(id, mouse.x, mouse.y) {
                    self.nodes[id]
                        .element
                        .select(Some(anchor.min(at)..anchor.max(at)));
                    self.dirty = true;
                    return Ok(Pointer::Selecting { changed: true });
                }
            }
        }
        match (mouse.kind, self.anchor) {
            (MouseKind::Down(_), _) => {
                self.release_pointer();
                changed = self.selection.is_some() || self.text_selection.is_some();
                self.clear_selection();
            }
            // A press that no node wanted is dragged into a selection.
            (MouseKind::Drag(_), Some(anchor)) => {
                self.selection = Some((anchor, (mouse.x, mouse.y)));
                self.dirty = true;
                return Ok(Pointer::Selecting { changed: true });
            }
            (MouseKind::Up(_), Some(_)) => {
                self.release_pointer();
                return Ok(Pointer::Selecting { changed: false });
            }
            // The target was resolved while the press still owned the pointer.
            (MouseKind::Up(_), None) => {
                self.release_pointer();
            }
            _ => {}
        }
        if target != self.hover {
            for (id, event) in [(self.hover, Event::Leave), (target, Event::Enter)] {
                if let Some(id) = id.filter(|id| self.contains(*id)) {
                    changed |= self.deliver(id, &event)?.changed;
                }
            }
            self.hover = target;
        }
        Ok(Pointer::Target { target, changed })
    }

    /// A press focuses the nearest focusable node at or above the target.
    /// The node is under the pointer already, so scrolling to show all of
    /// it, which focus otherwise asks for, would move it out from under it.
    fn focus_pressed(&mut self, target: Option<Id>) -> Result<(), Error> {
        let mut ancestor = target;
        while let Some(id) = ancestor {
            if self.nodes[id].element.focusable() {
                self.focus(Some(id))?;
                self.reveal_focus = false;
                break;
            }
            ancestor = self.nodes[id].parent;
        }
        Ok(())
    }

    /// Drop the focus from a node that is no longer displayed or focusable.
    fn drop_stale_focus(&mut self) -> Result<(), Error> {
        self.restore_scopes()?;
        if self.focus.is_some_and(|id| {
            !self.visible(id) || !self.in_scope(id) || !self.nodes[id].element.focusable()
        }) {
            self.focus(None)?;
        }
        Ok(())
    }

    pub fn dispatch(&mut self, event: Event) -> Result<Dispatch, Error> {
        let previous_focus = self.focus;
        self.drop_stale_focus()?;
        let mut target = self.target(&event);
        let mut changed = false;
        if let Event::Mouse(mouse) = &event {
            match self.pointer(mouse, target)? {
                Pointer::Selecting { changed } => {
                    return Ok(Dispatch {
                        handled: true,
                        changed,
                        ..Dispatch::default()
                    })
                }
                Pointer::Target {
                    target: to,
                    changed: moved,
                } => {
                    target = to;
                    changed = moved;
                }
            }
        }
        let press = match &event {
            Event::Mouse(mouse) => match mouse.kind {
                MouseKind::Down(button) => Some((button, (mouse.x, mouse.y))),
                _ => None,
            },
            _ => None,
        };
        if press.is_some() {
            self.focus_pressed(target)?;
        }
        let mut result = Dispatch {
            target,
            changed: changed || self.focus != previous_focus,
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
            next = if Some(id) == self.scope() {
                None
            } else {
                self.nodes[id].parent
            };
        }
        match press {
            // The node that handled a press owns the pointer until release.
            Some(_) if result.handled => self.capture = result.path.last().copied(),
            // A left press that nothing handled may become a selection.
            Some((crate::Button::Left, at)) if self.selectable => {
                let mut owner = target;
                while let Some(id) = owner {
                    if let Some(position) = self.logical_position(id, at.0, at.1) {
                        self.text_selection = Some(id);
                        self.text_anchor = Some(position);
                        self.nodes[id].element.select(Some(position..position));
                        break;
                    }
                    if Some(id) == self.scope() {
                        break;
                    }
                    owner = self.nodes[id].parent;
                }
                if self.text_selection.is_none() {
                    self.anchor = Some(at);
                }
            }
            _ => {}
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

    /// Translate a screen pointer into an element's stable content position.
    fn logical_position(&self, id: Id, x: u16, y: u16) -> Option<usize> {
        let node = self.nodes.get(id)?;
        let local = (
            (i32::from(x) - node.origin.0).clamp(0, i32::from(u16::MAX)) as u16,
            (i32::from(y) - node.origin.1).clamp(0, i32::from(u16::MAX)) as u16,
        );
        node.element.text_position(local, node.size)
    }
}

/// The outcome of the tree's own handling of a mouse event.
enum Pointer {
    /// The event extended or ended a screen selection and goes no further.
    Selecting { changed: bool },
    /// The event goes to `target`; `changed` reports a repaint already owed.
    Target { target: Option<Id>, changed: bool },
}
