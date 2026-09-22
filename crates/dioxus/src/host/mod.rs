//! Applies Dioxus mutations to the same tree used by imperative applications.
use dioxus_core::{
    AttributeValue, ElementId, Template, TemplateAttribute, TemplateNode, WriteMutations,
};
use std::collections::HashMap;
mod attrs;
use wove::{
    elements::{Container, Input, Panel, Scroll, Text, Textarea},
    Id, Layout, Tree,
};

#[derive(Debug)]
pub enum Error {
    Core(wove::Error),
    Unsupported(String),
    Protocol(&'static str),
    Poisoned,
}
impl From<wove::Error> for Error {
    fn from(e: wove::Error) -> Self {
        Self::Core(e)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Core(e) => e.fmt(f),
            Self::Unsupported(s) => write!(f, "unsupported element or attribute: {s}"),
            Self::Protocol(s) => write!(f, "invalid Dioxus mutation: {s}"),
            Self::Poisoned => f.write_str("view failed an earlier mutation"),
        }
    }
}
impl std::error::Error for Error {}

/// The listener names behind `elements::events`, without their `on` prefix.
const EVENTS: [&str; 4] = ["key", "paste", "mouse", "input"];

type Create = fn(&mut Tree) -> Result<Id, Error>;
type Set = fn(&mut Tree, Id, &str, &AttributeValue) -> Result<(), Error>;
/// Extra RSX tags can create any core element and define their own attributes.
/// The adapter applies layout attributes to every tag; a registered tag's `Set`
/// function receives all other attributes.
#[derive(Default)]
pub struct Registry {
    entries: HashMap<String, (Create, Set)>,
}
impl Registry {
    pub fn register(&mut self, tag: impl Into<String>, create: Create, set: Set) {
        self.entries.insert(tag.into(), (create, set));
    }
}

pub(crate) struct Host {
    pub tree: Tree,
    /// Dioxus reuses element ids, so a stale entry is overwritten before use.
    ids: HashMap<ElementId, Id>,
    stack: Vec<Id>,
    tags: HashMap<Id, String>,
    /// A node's layout before its first layout attribute, restored when one is removed.
    defaults: HashMap<Id, Layout>,
    listeners: HashMap<(Id, &'static str), ElementId>,
    registry: Registry,
    error: Option<Error>,
    failed: bool,
}
impl Host {
    pub fn new(registry: Registry) -> Self {
        let tree = Tree::new();
        let ids = HashMap::from([(ElementId(0), tree.root())]);
        Self {
            tree,
            ids,
            stack: vec![],
            tags: HashMap::new(),
            defaults: HashMap::new(),
            listeners: HashMap::new(),
            registry,
            error: None,
            failed: false,
        }
    }
    pub fn check(&mut self) -> Result<(), Error> {
        if let Some(e) = self.error.take() {
            return Err(e);
        }
        if self.failed {
            return Err(Error::Poisoned);
        }
        Ok(())
    }
    fn apply(&mut self, f: impl FnOnce(&mut Self) -> Result<(), Error>) {
        if self.failed {
            return;
        }
        if let Err(e) = f(self) {
            self.failed = true;
            self.error = Some(e);
        }
    }
    pub fn listener(&self, node: Id, name: &'static str) -> Option<ElementId> {
        self.listeners.get(&(node, name)).copied()
    }
    fn id(&self, id: ElementId) -> Result<Id, Error> {
        self.ids
            .get(&id)
            .copied()
            .ok_or(Error::Protocol("missing element"))
    }
    fn take(&mut self, m: usize) -> Result<Vec<Id>, Error> {
        let start = self
            .stack
            .len()
            .checked_sub(m)
            .ok_or(Error::Protocol("empty stack"))?;
        Ok(self.stack.split_off(start))
    }
    fn path(&self, path: &[u8]) -> Result<Id, Error> {
        let mut node = *self
            .stack
            .last()
            .ok_or(Error::Protocol("missing template root"))?;
        for i in path {
            node = *self
                .tree
                .children(node)?
                .get(usize::from(*i))
                .ok_or(Error::Protocol("invalid template path"))?;
        }
        Ok(node)
    }
    fn placeholder(&mut self) -> Result<Id, Error> {
        let id = self.tree.create(Container)?;
        self.tree.set_layout(
            id,
            Layout {
                display: wove::layout::Display::None,
                ..Layout::default()
            },
        )?;
        Ok(id)
    }
    fn template(&mut self, template: &TemplateNode) -> Result<Id, Error> {
        match template {
            TemplateNode::Text { text } => Ok(self.tree.create(Text::new(*text))?),
            TemplateNode::Dynamic { .. } => self.placeholder(),
            TemplateNode::Element {
                tag,
                namespace,
                attrs,
                children,
            } => {
                if namespace.is_some() {
                    return Err(Error::Unsupported(format!("namespace on {tag}")));
                }
                let id = match *tag {
                    "view" => self.tree.create(Container)?,
                    "panel" => self.tree.create(Panel::default())?,
                    "text" => self.tree.create(Text::default())?,
                    "input" => self.tree.create(Input::default())?,
                    "textarea" => self.tree.create(Textarea::default())?,
                    "scroll" => self.tree.create(Scroll::default())?,
                    tag => (self
                        .registry
                        .entries
                        .get(tag)
                        .ok_or_else(|| Error::Unsupported(tag.into()))?
                        .0)(&mut self.tree)?,
                };
                self.tags.insert(id, tag.to_string());
                for attr in *attrs {
                    if let TemplateAttribute::Static {
                        name,
                        value,
                        namespace,
                    } = attr
                    {
                        self.attr(
                            id,
                            name,
                            *namespace,
                            &AttributeValue::Text(value.to_string()),
                        )?;
                    }
                }
                for child in *children {
                    let child = self.template(child)?;
                    self.tree.append(id, child)?;
                }
                Ok(id)
            }
        }
    }
    /// Remove a subtree and forget only its nodes, so clearing a list stays linear.
    fn remove(&mut self, id: Id) -> Result<(), Error> {
        let mut nodes = vec![id];
        let mut next = 0;
        while next < nodes.len() {
            let children = self.tree.children(nodes[next])?;
            nodes.extend_from_slice(children);
            next += 1;
        }
        self.tree.remove(id)?;
        for node in nodes {
            self.tags.remove(&node);
            self.defaults.remove(&node);
            for name in EVENTS {
                self.listeners.remove(&(node, name));
            }
        }
        Ok(())
    }
    fn beside(&mut self, anchor: Id, nodes: Vec<Id>, after: bool) -> Result<(), Error> {
        let parent = self
            .tree
            .parent(anchor)
            .ok_or(Error::Protocol("detached anchor"))?;
        // Resolve the anchor after each move, since keyed siblings may already be in this parent.
        let mut previous = anchor;
        for node in nodes {
            let children = self.tree.children(parent)?;
            let index = children
                .iter()
                .position(|id| *id == previous)
                .ok_or(Error::Protocol("missing anchor"))?;
            let before = children
                .iter()
                .position(|id| *id == node)
                .is_some_and(|i| i < index);
            self.tree.insert(
                parent,
                node,
                index + usize::from(after) - usize::from(before),
            )?;
            if after {
                previous = node;
            }
        }
        Ok(())
    }
}

impl WriteMutations for Host {
    fn append_children(&mut self, id: ElementId, m: usize) {
        self.apply(|s| {
            let parent = s.id(id)?;
            for node in s.take(m)? {
                s.tree.append(parent, node)?;
            }
            Ok(())
        });
    }
    fn assign_node_id(&mut self, path: &'static [u8], id: ElementId) {
        self.apply(|s| {
            let node = s.path(path)?;
            s.ids.insert(id, node);
            Ok(())
        });
    }
    fn create_placeholder(&mut self, id: ElementId) {
        self.apply(|s| {
            let node = s.placeholder()?;
            s.ids.insert(id, node);
            s.stack.push(node);
            Ok(())
        });
    }
    fn create_text_node(&mut self, value: &str, id: ElementId) {
        self.apply(|s| {
            let node = s.tree.create(Text::new(value))?;
            s.ids.insert(id, node);
            s.stack.push(node);
            Ok(())
        });
    }
    fn load_template(&mut self, template: Template, index: usize, id: ElementId) {
        self.apply(|s| {
            let node = s.template(
                template
                    .roots
                    .get(index)
                    .ok_or(Error::Protocol("missing template"))?,
            )?;
            s.ids.insert(id, node);
            s.stack.push(node);
            Ok(())
        });
    }
    fn replace_node_with(&mut self, id: ElementId, m: usize) {
        self.apply(|s| {
            let anchor = s.id(id)?;
            let nodes = s.take(m)?;
            s.beside(anchor, nodes, false)?;
            s.remove(anchor)
        });
    }
    fn replace_placeholder_with_nodes(&mut self, path: &'static [u8], m: usize) {
        self.apply(|s| {
            let nodes = s.take(m)?;
            let anchor = s.path(path)?;
            s.beside(anchor, nodes, false)?;
            s.remove(anchor)
        });
    }
    fn insert_nodes_after(&mut self, id: ElementId, m: usize) {
        self.apply(|s| {
            let anchor = s.id(id)?;
            let nodes = s.take(m)?;
            s.beside(anchor, nodes, true)
        });
    }
    fn insert_nodes_before(&mut self, id: ElementId, m: usize) {
        self.apply(|s| {
            let anchor = s.id(id)?;
            let nodes = s.take(m)?;
            s.beside(anchor, nodes, false)
        });
    }
    fn set_attribute(
        &mut self,
        name: &'static str,
        ns: Option<&'static str>,
        value: &AttributeValue,
        id: ElementId,
    ) {
        self.apply(|s| s.attr(s.id(id)?, name, ns, value));
    }
    fn set_node_text(&mut self, value: &str, id: ElementId) {
        self.apply(|s| {
            s.tree
                .update::<Text>(s.id(id)?, |w| w.content = value.into())?;
            Ok(())
        });
    }
    fn create_event_listener(&mut self, name: &'static str, id: ElementId) {
        self.apply(|s| {
            if !EVENTS.contains(&name) {
                return Err(Error::Unsupported(format!("event {name}")));
            }
            s.listeners.insert((s.id(id)?, name), id);
            Ok(())
        });
    }
    fn remove_event_listener(&mut self, name: &'static str, id: ElementId) {
        self.apply(|s| {
            s.listeners.remove(&(s.id(id)?, name));
            Ok(())
        });
    }
    fn remove_node(&mut self, id: ElementId) {
        self.apply(|s| s.remove(s.id(id)?));
    }
    fn push_root(&mut self, id: ElementId) {
        self.apply(|s| {
            s.stack.push(s.id(id)?);
            Ok(())
        });
    }
}
