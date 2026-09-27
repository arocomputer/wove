//! Two-axis virtualization implemented entirely through the public child API.
use wove::{
    elements::Text, terminal, Children, Element, Error, Event, Key, Layout, Response, Tree,
};

/// A fixed-cell data grid. Only visible cells are measured and painted.
struct Grid {
    row: usize,
    column: usize,
    columns: usize,
}
impl Element for Grid {
    fn focusable(&self) -> bool {
        true
    }
    fn manages_children(&self) -> bool {
        true
    }
    fn layout(&self) -> Layout {
        Layout {
            flex_grow: 1.0,
            ..Layout::default()
        }
    }
    fn arrange(&mut self, children: &mut dyn Children) -> Result<(), Error> {
        if let Some(index) = children.focused() {
            self.row = index / self.columns;
            self.column = index % self.columns;
        }
        let (width, height) = children.size();
        for y in 0..usize::from(height) {
            for x in 0..usize::from(width).div_ceil(12) {
                let column = self.column + x;
                let index = (self.row + y) * self.columns + column;
                if column < self.columns && index < children.ids().len() {
                    children.place(index, ((x * 12) as i32, y as i32), 12)?;
                }
            }
        }
        Ok(())
    }
    fn event(&mut self, event: &Event) -> Response {
        match event {
            Event::Key(Key::Up, _) => self.row = self.row.saturating_sub(1),
            Event::Key(Key::Down, _) => self.row = (self.row + 1).min(999),
            Event::Key(Key::Left, _) => self.column = self.column.saturating_sub(1),
            Event::Key(Key::Right, _) => self.column = (self.column + 1).min(self.columns - 1),
            _ => return Response::IGNORE,
        }
        Response::REPAINT
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut tree = Tree::new();
    tree.add(
        tree.root(),
        Text::new("Wove grid · arrows scroll · Esc quits"),
    )?;
    let grid = tree.add(
        tree.root(),
        Grid {
            row: 0,
            column: 0,
            columns: 100,
        },
    )?;
    for row in 0..1000 {
        for column in 0..100 {
            tree.add(grid, Text::new(format!("{row:04}:{column:03}")))?;
        }
    }
    tree.focus(Some(grid))?;
    terminal::run(&mut tree, |_, event, _| {
        !matches!(event, Event::Key(Key::Escape, _))
    })?;
    Ok(())
}
