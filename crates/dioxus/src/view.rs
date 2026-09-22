//! Component ownership and event routing into the core tree.
use crate::host;
use crate::{Error, Registry};
use dioxus_core::{Event as UiEvent, VirtualDom};
use std::{any::Any, rc::Rc};
use wove::{Buffer, Dispatch, Event, Tree};

/// Owns a Dioxus application and its terminal nodes. The caller owns scheduling.
pub struct View {
    dom: VirtualDom,
    host: host::Host,
}
impl View {
    pub fn new(dom: VirtualDom) -> Result<Self, Error> {
        Self::with_registry(dom, Registry::default())
    }
    pub fn with_registry(mut dom: VirtualDom, registry: Registry) -> Result<Self, Error> {
        let mut host = host::Host::new(registry);
        dom.rebuild(&mut host);
        host.check()?;
        Ok(Self { dom, host })
    }
    pub fn tree(&self) -> &Tree {
        &self.host.tree
    }
    pub fn focus(&mut self, id: Option<wove::Id>) -> Result<(), Error> {
        self.host.check()?;
        self.host.tree.focus(id)?;
        Ok(())
    }
    pub fn focus_next(&mut self, reverse: bool) -> Result<(), Error> {
        self.host.check()?;
        self.host.tree.focus_next(reverse)?;
        Ok(())
    }
    /// Apply scheduled component updates. A failed mutation makes the view unusable.
    pub fn render(&mut self) -> Result<(), Error> {
        self.host.check()?;
        self.dom.render_immediate(&mut self.host);
        self.host.check()
    }
    pub fn frame(&mut self, width: u16, height: u16) -> Result<&Buffer, Error> {
        self.render()?;
        Ok(self.host.tree.frame(width, height)?)
    }
    /// Dispatch key, paste, and mouse events to Dioxus listeners before native
    /// element behavior; other events go to the tree only. `prevent_default` cancels
    /// editing or focus traversal; `stop_propagation` stops Dioxus parent listeners.
    /// Mouse listeners follow native pointer capture. A prevented release still
    /// ends the gesture, so later events cannot remain captured by that node.
    pub fn send(&mut self, event: Event) -> Result<Dispatch, Error> {
        self.render()?;
        let target = self.host.tree.target(&event);
        // Other events have no Dioxus listener and go straight to the tree.
        let name = match event {
            Event::Key(..) => Some("key"),
            Event::Paste(..) => Some("paste"),
            Event::Mouse(..) => Some("mouse"),
            _ => None,
        };
        if name.is_some_and(|name| !self.emit(target, name, Rc::new(event.clone()))) {
            if matches!(&event, Event::Mouse(mouse) if matches!(mouse.kind, wove::MouseKind::Down(_) | wove::MouseKind::Up(_)))
            {
                self.host.tree.release_pointer();
            }
            self.render()?;
            return Ok(Dispatch {
                target,
                handled: true,
                ..Dispatch::default()
            });
        }
        // Snapshot a value only when a listener would hear that it changed.
        let ancestors = |id| std::iter::successors(id, |id| self.host.tree.parent(*id));
        let heard = |id, name| ancestors(Some(id)).any(|id| self.host.listener(id, name).is_some());
        let input = ancestors(target)
            .find(|id| self.input_value(*id).is_some())
            .filter(|id| heard(*id, "input"))
            .and_then(|id| self.input_value(id).map(|value| (id, value.to_owned())));
        let select = ancestors(target)
            .find(|id| self.selected(*id).is_some())
            .filter(|id| heard(*id, "select"))
            .and_then(|id| self.selected(id).map(|index| (id, index)));
        let result = self.host.tree.dispatch(event)?;
        if let Some((id, before)) = input {
            if let Some(value) = self.input_value(id) {
                if value != before {
                    self.emit(Some(id), "input", Rc::new(value.to_owned()));
                }
            }
        }
        if let Some((id, before)) = select {
            if let Some(index) = self.selected(id).filter(|index| *index != before) {
                self.emit(Some(id), "select", Rc::new(index));
            }
        }
        self.render()?;
        Ok(result)
    }
    /// Both editable elements share the same value-change event.
    fn input_value(&self, id: wove::Id) -> Option<&str> {
        use wove::elements::{Input, Textarea};
        self.host
            .tree
            .get::<Input>(id)
            .map(|input| input.editor.text())
            .ok()
            .or_else(|| {
                self.host
                    .tree
                    .get::<Textarea>(id)
                    .map(|area| area.editor.text())
                    .ok()
            })
    }
    /// The selected row of a list or table, which share the selection event.
    fn selected(&self, id: wove::Id) -> Option<usize> {
        use wove::elements::{List, Table};
        let tree = &self.host.tree;
        tree.get::<List>(id)
            .map(|list| list.selected)
            .or_else(|_| tree.get::<Table>(id).map(|table| table.selected))
            .ok()
    }
    fn emit(&self, target: Option<wove::Id>, name: &'static str, data: Rc<dyn Any>) -> bool {
        let mut node = target;
        while let Some(id) = node {
            if let Some(element) = self.host.listener(id, name) {
                let ui = UiEvent::new(data, true);
                self.dom.runtime().handle_event(name, ui.clone(), element);
                return ui.default_action_enabled();
            }
            node = self.host.tree.parent(id);
        }
        true
    }
    /// Wait for a signal or task to schedule work. Combine with terminal input in
    /// the application's executor; the adapter does not impose a runtime.
    pub async fn wait_for_work(&mut self) {
        self.dom.wait_for_work().await;
    }
}
