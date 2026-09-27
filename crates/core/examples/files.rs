//! A synthetic file browser with a modal inspector and restored navigation focus.
use wove::{
    elements::{Input, List, Panel, Text},
    layout::{length, Position, Size},
    terminal,
    text::Span,
    Event, Key, Layout, Style, Tree,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut tree = Tree::new();
    tree.add(
        tree.root(),
        Text::new("Wove files · Enter inspects · Esc closes"),
    )?;
    let list = tree.add(
        tree.root(),
        List::new(200, 32, |index| {
            vec![Span::new(
                format!("src/module_{index:03}.rs"),
                Style::default(),
            )]
        }),
    )?;
    tree.focus(Some(list))?;
    let mut dialog = None;
    let mut failure = None;
    terminal::run(&mut tree, |tree, event, _| {
        let result = (|| -> Result<bool, wove::Error> {
            match event {
                Event::Key(Key::Enter, _) if dialog.is_none() => {
                    let index = tree.get::<List>(list)?.selected;
                    // The logical parent owns the inspector even though it paints
                    // outside the list's clip and uses root-relative coordinates.
                    let panel = tree.add(
                        list,
                        Panel {
                            title: "Inspector".into(),
                            style: Style {
                                bg: wove::Color::Indexed(236),
                                ..Style::default()
                            },
                            ..Panel::default()
                        },
                    )?;
                    tree.set_overlay(panel, true)?;
                    tree.set_layout(
                        panel,
                        Layout {
                            position: Position::Absolute,
                            inset: wove::layout::Rect {
                                left: length(4.0),
                                top: length(3.0),
                                right: wove::layout::auto(),
                                bottom: wove::layout::auto(),
                            },
                            size: Size {
                                width: length(30.0),
                                height: length(5.0),
                            },
                            border: wove::layout::Rect::length(1.0),
                            flex_direction: wove::layout::FlexDirection::Column,
                            ..Layout::default()
                        },
                    )?;
                    tree.add(panel, Text::new(format!("module_{index:03}.rs")))?;
                    tree.add(panel, Input::new("Editable note"))?;
                    tree.push_scope(panel)?;
                    dialog = Some(panel);
                }
                Event::Key(Key::Escape, _) => {
                    if let Some(panel) = dialog.take() {
                        tree.remove(panel)?;
                    } else {
                        return Ok(false);
                    }
                }
                _ => {}
            }
            Ok(true)
        })();
        match result {
            Ok(keep) => keep,
            Err(error) => {
                failure = Some(error);
                false
            }
        }
    })?;
    if let Some(error) = failure {
        return Err(error.into());
    }
    Ok(())
}
