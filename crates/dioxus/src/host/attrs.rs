//! Typed RSX attributes and their native element defaults.
use super::{Error, Host};
use dioxus_core::AttributeValue;
use std::any::Any;
use wove::{
    elements::{Input, Lazy, List, Panel, RichText, Scroll, Table, Text, Textarea},
    render::columns,
    text::{clean, Span, Wrap},
    Id, Layout, Style,
};

/// Attributes the adapter applies to the layout of every tag, including registered ones.
const LAYOUT: [&str; 7] = [
    "layout",
    "width",
    "height",
    "grow",
    "gap",
    "padding",
    "direction",
];

impl Host {
    pub(super) fn attr(
        &mut self,
        id: Id,
        name: &str,
        ns: Option<&str>,
        value: &AttributeValue,
    ) -> Result<(), Error> {
        if ns.is_some() {
            return Err(Error::Unsupported(format!("namespace on {name}")));
        }
        if LAYOUT.contains(&name) {
            return self.layout_attr(id, name, value);
        }
        if let Some(tag) = self.tags.get(&id) {
            if let Some((_, set)) = self.registry.entries.get(tag) {
                return set(&mut self.tree, id, name, value);
            }
        }
        if name == "style" {
            return self.style_attr(id, value);
        }
        let tag = self.tags.get(&id).map(String::as_str);
        match (tag, name) {
            (Some("list"), "rows") => {
                let rows: Vec<Vec<Span>> = typed(name, value)?;
                // A list measures as wide as its widest row.
                let width = rows
                    .iter()
                    .map(|row| row.iter().map(|span| columns(&span.text)).sum::<usize>())
                    .max()
                    .unwrap_or(0);
                let count = rows.len();
                *self.lists[&id].borrow_mut() = rows;
                self.tree.update::<List>(id, |list| {
                    list.count = count;
                    list.width = u16::try_from(width).unwrap_or(u16::MAX);
                })?;
                return Ok(());
            }
            (Some("table"), "rows") => {
                let rows = typed(name, value)?;
                self.tree.update::<Table>(id, |table| table.rows = rows)?;
                return Ok(());
            }
            (Some("table"), "columns") => {
                let columns = typed(name, value)?;
                self.tree
                    .update::<Table>(id, |table| table.columns = columns)?;
                return Ok(());
            }
            (Some("rich"), "spans") => {
                let spans = typed(name, value)?;
                self.tree
                    .update::<RichText>(id, |rich| rich.spans = spans)?;
                return Ok(());
            }
            _ => {}
        }
        let tag = tag.map(str::to_owned);
        self.scalar_attr(id, tag.as_deref(), name, value)
    }

    /// A `Style` value on an element that has one.
    fn style_attr(&mut self, id: Id, value: &AttributeValue) -> Result<(), Error> {
        let style = match value {
            AttributeValue::Any(value) => *value
                .as_any()
                .downcast_ref::<Style>()
                .ok_or_else(|| Error::Unsupported("style expects Style".into()))?,
            AttributeValue::None => Style::default(),
            _ => return Err(Error::Unsupported("style expects Style".into())),
        };
        match self.tags.get(&id).map(String::as_str) {
            Some("text") => self.tree.update::<Text>(id, |w| w.style = style)?,
            Some("input") => self.tree.update::<Input>(id, |w| w.style = style)?,
            Some("textarea") => self.tree.update::<Textarea>(id, |w| w.style = style)?,
            Some("panel") => self.tree.update::<Panel>(id, |w| w.style = style)?,
            Some("table") => self.tree.update::<Table>(id, |w| w.style = style)?,
            _ => {
                return Err(Error::Unsupported(
                    "style requires text, input, textarea, panel, or table".into(),
                ))
            }
        }
        Ok(())
    }

    /// An attribute given as text, a number, or a bool, or removed.
    fn scalar_attr(
        &mut self,
        id: Id,
        tag: Option<&str>,
        name: &str,
        value: &AttributeValue,
    ) -> Result<(), Error> {
        let (text, absent) = scalar(name, value)?;
        let textarea = self.tags.get(&id).is_some_and(|tag| tag == "textarea");
        match name {
            "content" => self.tree.update::<Text>(id, |w| w.content = text)?,
            // Compare the stored form, so an equivalent value keeps cursor and undo.
            "value" if textarea => self.tree.update::<Textarea>(id, |area| {
                let text = clean(&text, true);
                if area.editor.text() != text {
                    area.editor.set(text);
                }
            })?,
            "value" => self.tree.update::<Input>(id, |input| {
                let text = clean(&text, false);
                if input.editor.text() != text {
                    input.editor.set(text);
                }
            })?,
            "placeholder" if textarea => self
                .tree
                .update::<Textarea>(id, |area| area.placeholder = text)?,
            "placeholder" => self.tree.update::<Input>(id, |w| w.placeholder = text)?,
            "wrap" | "follow" => {
                let on = if absent {
                    false
                } else {
                    text.parse()
                        .map_err(|_| Error::Unsupported(format!("{name}={text:?}")))?
                };
                match (tag, name) {
                    (Some("rich"), "wrap") => self.tree.update::<RichText>(id, |w| {
                        w.wrap = if on { Wrap::Word } else { Wrap::None }
                    })?,
                    (Some("text"), "wrap") => self.tree.update::<Text>(id, |w| w.wrap = on)?,
                    (Some("textarea"), "wrap") => {
                        self.tree.update::<Textarea>(id, |w| w.wrap = on)?
                    }
                    (Some("lazy"), "follow") => self.tree.update::<Lazy>(id, |w| w.follow = on)?,
                    (Some("scroll"), "follow") => {
                        self.tree.update::<Scroll>(id, |w| w.follow = on)?
                    }
                    _ => {
                        let tag = tag.unwrap_or("a custom element");
                        return Err(Error::Unsupported(format!("{tag} has no {name}")));
                    }
                }
            }
            "selected" => {
                let selected = if absent {
                    0
                } else {
                    text.parse()
                        .map_err(|_| Error::Unsupported(format!("{name}={text:?}")))?
                };
                match tag {
                    Some("table") => self.tree.update::<Table>(id, |w| w.selected = selected)?,
                    _ => self.tree.update::<List>(id, |w| w.selected = selected)?,
                }
            }
            _ => return Err(Error::Unsupported(format!("{name}={text:?}"))),
        }
        Ok(())
    }

    /// Apply one of `LAYOUT`. Removing an attribute restores that part of the
    /// layout the node's element had before any layout attribute was set.
    fn layout_attr(&mut self, id: Id, name: &str, value: &AttributeValue) -> Result<(), Error> {
        if !self.defaults.contains_key(&id) {
            let layout = self.tree.layout(id)?.clone();
            self.defaults.insert(id, layout);
        }
        let default = &self.defaults[&id];
        if name == "layout" {
            let layout = match value {
                AttributeValue::Any(value) => value
                    .as_any()
                    .downcast_ref::<Layout>()
                    .cloned()
                    .ok_or_else(|| Error::Unsupported("layout expects Layout".into()))?,
                AttributeValue::None => default.clone(),
                _ => return Err(Error::Unsupported("layout expects Layout".into())),
            };
            self.tree.set_layout(id, layout)?;
            return Ok(());
        }
        let (text, absent) = scalar(name, value)?;
        let invalid = || Error::Unsupported(format!("{name}={text:?}"));
        let mut style = self.tree.layout(id)?.clone();
        use wove::layout::{length, FlexDirection, Rect, Size};
        if absent {
            match name {
                "width" => style.size.width = default.size.width,
                "height" => style.size.height = default.size.height,
                "grow" => style.flex_grow = default.flex_grow,
                "gap" => style.gap = default.gap,
                "padding" => style.padding = default.padding,
                _ => style.flex_direction = default.flex_direction,
            }
        } else if name == "direction" {
            style.flex_direction = match text.as_str() {
                "row" => FlexDirection::Row,
                "column" => FlexDirection::Column,
                _ => return Err(invalid()),
            };
        } else {
            let n = text.parse::<f32>().map_err(|_| invalid())?;
            if !n.is_finite() || n < 0.0 {
                return Err(invalid());
            }
            match name {
                "width" => style.size.width = length(n),
                "height" => style.size.height = length(n),
                "grow" => style.flex_grow = n,
                "gap" => {
                    style.gap = Size {
                        width: length(n),
                        height: length(n),
                    }
                }
                _ => style.padding = Rect::length(n),
            }
        }
        self.tree.set_layout(id, style)?;
        Ok(())
    }
}

/// The text of a scalar attribute, and whether the attribute was removed.
fn scalar(name: &str, value: &AttributeValue) -> Result<(String, bool), Error> {
    Ok(match value {
        AttributeValue::Text(s) => (s.clone(), false),
        AttributeValue::Int(n) => (n.to_string(), false),
        AttributeValue::Float(n) => (n.to_string(), false),
        AttributeValue::Bool(b) => (b.to_string(), false),
        AttributeValue::None => (String::new(), true),
        _ => return Err(Error::Unsupported(name.into())),
    })
}

/// A value passed with `AttributeValue::any_value`, or the type's default when
/// the attribute is removed.
fn typed<T: Any + Clone + Default>(name: &str, value: &AttributeValue) -> Result<T, Error> {
    let expected = || Error::Unsupported(format!("{name} expects {}", std::any::type_name::<T>()));
    match value {
        AttributeValue::Any(value) => value
            .as_any()
            .downcast_ref::<T>()
            .cloned()
            .ok_or_else(expected),
        AttributeValue::None => Ok(T::default()),
        _ => Err(expected()),
    }
}
