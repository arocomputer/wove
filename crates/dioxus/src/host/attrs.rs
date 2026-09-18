//! Typed RSX attributes and their native element defaults.
use super::{Error, Host};
use dioxus_core::AttributeValue;
use wove::{
    elements::{Input, Panel, Scroll, Text, Textarea},
    Element, Id, Layout, Style,
};
impl Host {
    fn default_layout(&self, id: Id) -> Layout {
        match self.tags.get(&id).map(String::as_str) {
            Some("input") => Input::default().layout(),
            Some("panel") => Panel::default().layout(),
            Some("scroll") => Scroll::default().layout(),
            _ => Layout::default(),
        }
    }
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
        if let Some(tag) = self.tags.get(&id) {
            if let Some((_, set)) = self.registry.entries.get(tag) {
                return set(&mut self.tree, id, name, value);
            }
        }
        if name == "layout" {
            let style = match value {
                AttributeValue::Any(value) => value
                    .as_any()
                    .downcast_ref::<Layout>()
                    .cloned()
                    .ok_or_else(|| Error::Unsupported("layout expects Layout".into()))?,
                AttributeValue::None => self.default_layout(id),
                _ => return Err(Error::Unsupported("layout expects Layout".into())),
            };
            self.tree.set_layout(id, style)?;
            return Ok(());
        }
        if name == "style" {
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
                _ => {
                    return Err(Error::Unsupported(
                        "style requires text, input, textarea, or panel".into(),
                    ))
                }
            }
            return Ok(());
        }
        let text = match value {
            AttributeValue::Text(s) => s.clone(),
            AttributeValue::Int(n) => n.to_string(),
            AttributeValue::Float(n) => n.to_string(),
            AttributeValue::Bool(b) => b.to_string(),
            AttributeValue::None => String::new(),
            _ => return Err(Error::Unsupported(name.into())),
        };
        let absent = matches!(value, AttributeValue::None);
        let invalid = || Error::Unsupported(format!("{name}={text:?}"));
        match name {
            "content" => self.tree.update::<Text>(id, |w| w.content = text)?,
            "value" if self.tags.get(&id).is_some_and(|tag| tag == "textarea") => {
                self.tree.update::<Textarea>(id, |area| {
                    let replacement = Textarea::new(&text);
                    if area.editor.text() != replacement.editor.text() {
                        area.editor = replacement.editor;
                    }
                })?;
            }
            "placeholder" if self.tags.get(&id).is_some_and(|tag| tag == "textarea") => {
                self.tree
                    .update::<Textarea>(id, |area| area.placeholder = text)?
            }
            "value" => self.tree.update::<Input>(id, |w| {
                if w.editor.text() != text {
                    w.editor
                        .set(text.chars().filter(|c| !c.is_control()).collect::<String>());
                }
            })?,
            "placeholder" => self.tree.update::<Input>(id, |w| w.placeholder = text)?,
            "wrap" => {
                let wrap = if absent {
                    false
                } else {
                    text.parse().map_err(|_| invalid())?
                };
                self.tree.update::<Text>(id, |w| w.wrap = wrap)?;
            }
            "width" | "height" | "grow" | "gap" | "padding" | "direction" => {
                let mut style = self.tree.layout(id)?.clone();
                if name == "direction" {
                    style.flex_direction = match text.as_str() {
                        "" if absent => self.default_layout(id).flex_direction,
                        "row" => wove::layout::FlexDirection::Row,
                        "column" => wove::layout::FlexDirection::Column,
                        _ => return Err(invalid()),
                    };
                } else {
                    let n = if absent {
                        0.0
                    } else {
                        text.parse::<f32>().map_err(|_| invalid())?
                    };
                    if !n.is_finite() || n < 0.0 {
                        return Err(invalid());
                    }
                    use wove::layout::*;
                    match name {
                        "width" => {
                            style.size.width = if absent {
                                self.default_layout(id).size.width
                            } else {
                                length(n)
                            }
                        }
                        "height" => {
                            style.size.height = if absent {
                                self.default_layout(id).size.height
                            } else {
                                length(n)
                            }
                        }
                        "grow" => style.flex_grow = n,
                        "gap" => {
                            style.gap = Size {
                                width: length(n),
                                height: length(n),
                            }
                        }
                        "padding" => style.padding = Rect::length(n),
                        _ => unreachable!(),
                    }
                }
                self.tree.set_layout(id, style)?;
            }
            _ => return Err(invalid()),
        }
        Ok(())
    }
}
