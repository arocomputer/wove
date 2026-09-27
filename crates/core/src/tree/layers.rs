//! Overlays keep logical parents; focus scopes bound input and restore focus.
use super::{Error, Id, Tree};

impl Tree {
    /// Render a subtree above normal content, positioned against the root rather
    /// than its logical parent. Layout becomes absolute when enabled. The node
    /// keeps its parent for visibility, ownership, focus order, and bubbling.
    /// Set layout insets to position it; `z` orders overlays amongst themselves.
    /// Disabling retains its layout style, so callers may choose new positioning.
    pub fn set_overlay(&mut self, id: Id, enabled: bool) -> Result<(), Error> {
        self.node(id)?;
        if id == self.root {
            return Err(Error::Root);
        }
        if self.nodes[id].overlay == enabled {
            return Ok(());
        }
        let node = self.nodes[id].layout;
        if let Some(parent) = self.layout.parent(node) {
            self.layout.remove_child(parent, node)?;
        }
        self.nodes[id].overlay = enabled;
        self.overlays.retain(|other| *other != id);
        if enabled {
            let mut style = self.layout.style(node)?.clone();
            style.position = taffy::Position::Absolute;
            self.layout.set_style(node, style)?;
            self.layout.add_child(self.nodes[self.root].layout, node)?;
            self.overlays.push(id);
            self.sort_overlays();
        } else if let Some(parent) = self.nodes[id].parent {
            if !self.managed(parent) {
                let index = self.nodes[parent]
                    .children
                    .iter()
                    .take_while(|child| **child != id)
                    .filter(|child| !self.nodes[**child].overlay)
                    .count();
                self.layout
                    .insert_child_at_index(self.nodes[parent].layout, index, node)?;
            }
        }
        self.hide(id);
        self.touch();
        Ok(())
    }

    /// Whether a subtree participates in pointer hit testing. A decorative
    /// overlay may pass through clicks while retaining keyboard focus behavior.
    pub fn set_pointer_events(&mut self, id: Id, enabled: bool) -> Result<(), Error> {
        self.nodes
            .get_mut(id)
            .ok_or(Error::MissingNode)?
            .pointer_events = enabled;
        self.release_pointer();
        Ok(())
    }

    /// Restrict keyboard focus and pointer input to a displayed subtree until
    /// popped, hidden, or removed. Unhandled events stop at the scope root.
    /// Nested scopes remember and restore their previously focused nodes.
    pub fn push_scope(&mut self, id: Id) -> Result<(), Error> {
        self.node(id)?;
        if !self.visible(id) {
            return Err(Error::MissingNode);
        }
        self.scopes.push((id, self.focus));
        self.release_pointer();
        self.clear_selection();
        self.focus(None)?;
        self.focus_next(false)
    }

    /// Close the latest focus scope, restoring a still-visible previous focus.
    pub fn pop_scope(&mut self) -> Result<Option<Id>, Error> {
        let Some((id, saved)) = self.scopes.pop() else {
            return Ok(None);
        };
        self.release_pointer();
        self.focus(None)?;
        if let Some(saved) = saved.filter(|saved| self.visible(*saved) && self.in_scope(*saved)) {
            self.focus(Some(saved))?;
        }
        Ok(Some(id))
    }

    /// The current input boundary, independent of painting order.
    pub fn scope(&self) -> Option<Id> {
        self.scopes.last().map(|scope| scope.0)
    }

    pub(super) fn sort_overlays(&mut self) {
        let nodes = &self.nodes;
        self.overlays.sort_by_key(|id| nodes[*id].z);
    }
    pub(super) fn in_scope(&self, id: Id) -> bool {
        let Some(scope) = self.scope() else {
            return true;
        };
        let mut at = Some(id);
        while let Some(id) = at {
            if id == scope {
                return true;
            }
            at = self.nodes.get(id).and_then(|node| node.parent);
        }
        false
    }
    /// Pointer transparency applies through logical parents, including overlays.
    pub(super) fn pointer_enabled(&self, id: Id) -> bool {
        let mut at = Some(id);
        while let Some(id) = at {
            let Some(node) = self.nodes.get(id) else {
                return false;
            };
            if !node.pointer_events {
                return false;
            }
            at = node.parent;
        }
        true
    }
    pub(super) fn restore_scopes(&mut self) -> Result<(), Error> {
        // Hiding an outer scope closes every scope opened above it.
        if let Some(index) = self.scopes.iter().position(|(id, _)| !self.visible(*id)) {
            while self.scopes.len() > index {
                self.pop_scope()?;
            }
        }
        Ok(())
    }
}
