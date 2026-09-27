//! A virtualized column of element subtrees, laid out only where in view.
use crate::{Children, Element, Error, Event, Id, Key, Layout, MouseKind, Response};

/// A long column of element subtrees, such as messages that each carry their
/// own header and controls, or the entries of a file tree. The tree lays out
/// only the children in view, so a frame costs what is visible however many
/// children there are. Use a `Feed` when the content is text alone: it skips
/// element layout altogether.
///
/// Children stack from the top in child order, and `z` does not reorder them.
/// The tree lays out each child on its own, as the only child of a column as
/// wide as this one's content, and keeps that layout until something in the
/// child changes. A child is as tall as its content at that width, margins
/// included; percentage heights and growing have no height to fill.
///
/// Positions are a child and a row within it rather than a row count, because
/// the rows above the view are never measured. The view holds its first child
/// in place while others arrive or leave around it. With `follow`, it shows the
/// tail and stays there as children arrive; scrolling away clears `follow`, and
/// scrolling back to the tail sets it again. A descendant that receives focus
/// is scrolled into view.
///
/// Like a `Feed`, it has no height of its own. It grows into the space its
/// parent gives it, so in a frame of natural height, such as an inline session,
/// give it a height in its layout.
#[derive(Default)]
pub struct Lazy {
    pub follow: bool,
    /// The first visible row, as the child it is in, that child's index when
    /// last painted, and a row within it. `None` is the first row.
    top: Option<(Id, usize, usize)>,
    /// Rows to scroll by at the next paint, when children can be measured.
    pending: isize,
    /// Rows of a child to bring into view at the next paint: the child's
    /// index, the first of the rows, and how many.
    reveal: Option<(usize, usize, usize)>,
    /// Whether the last paint showed the first row and the tail.
    ends: (bool, bool),
    /// The height of the last paint, which scrolling by pages is measured in.
    height: u16,
}

impl Lazy {
    /// Ask the next paint to show `size` rows of the child at `index`,
    /// starting at `row`. The tree calls it when focus moves inside the child.
    fn show(&mut self, index: usize, row: usize, size: usize) {
        self.reveal = Some((index, row, size));
    }

    /// Settle the view for a paint `height` rows tall over nonempty children, and
    /// return the first visible child and how many of its rows are above the
    /// view. `rows` lays out a child and returns its height; it is only asked
    /// about the children near the view and those a scroll passes.
    fn settle(
        &mut self,
        height: u16,
        count: usize,
        anchor: Option<(usize, usize)>,
        rows: &mut dyn FnMut(usize) -> usize,
    ) -> (usize, usize) {
        self.height = height;
        let height = usize::from(height);
        let tail = super::tail(count, height, rows);
        let mut at = match anchor {
            _ if self.follow => tail,
            None => (0, 0),
            Some((index, row)) => {
                // A child laid out again at a new width may have fewer rows
                // than the anchor remembers; hold its last row instead.
                (index, row.min(rows(index).saturating_sub(1))).min(tail)
            }
        };
        let moved = self.pending != 0 || self.reveal.is_some();
        if self.pending != 0 {
            at = super::step(at, std::mem::take(&mut self.pending), count, rows).min(tail);
        }
        if let Some(target) = self.reveal.take().filter(|(index, ..)| *index < count) {
            at = reveal(at, target, height, count, rows).min(tail);
        }
        if moved {
            self.follow = at >= tail;
        }
        self.ends = (at == (0, 0), at >= tail);
        at
    }
}

/// The first row of a view `height` rows tall, starting from `at`, that shows
/// `size` rows of child `index` from `row`. The view moves only as far as it
/// must, and shows the first of the rows when they do not all fit.
fn reveal(
    at: (usize, usize),
    (index, row, size): (usize, usize, usize),
    height: usize,
    count: usize,
    rows: &mut dyn FnMut(usize) -> usize,
) -> (usize, usize) {
    let top = (index, row);
    if top < at {
        return top;
    }
    // Rows from the top of the first visible child to the target's child,
    // measured no further than the view reaches.
    let (mut child, mut above) = (at.0, 0);
    while child < index && above <= at.1 + height {
        above += rows(child);
        child += 1;
    }
    if child == index && above + row + size <= at.1 + height {
        return at;
    }
    let height = isize::try_from(height).unwrap_or(isize::MAX);
    super::step((index, row + size), -height, count, rows).min(top)
}

impl Element for Lazy {
    fn manages_children(&self) -> bool {
        true
    }
    fn arrange(&mut self, children: &mut dyn Children) -> Result<(), Error> {
        let (width, height) = children.size();
        if let Some(focus) = children.reveal(width)? {
            self.show(focus.index, focus.at.1 as usize, usize::from(focus.size.1));
        }
        let count = children.ids().len();
        if count == 0 {
            self.top = None;
            self.pending = 0;
            self.reveal = None;
            self.height = height;
            self.ends = (true, true);
            return Ok(());
        }
        let anchor = self.top.map(|(id, hint, row)| {
            let ids = children.ids();
            let index = if ids.get(hint) == Some(&id) {
                hint
            } else {
                ids.iter()
                    .position(|child| *child == id)
                    .unwrap_or(hint.min(count - 1))
            };
            (index, row)
        });
        let mut failed = None;
        let (first, skipped) = self.settle(height, count, anchor, &mut |index| {
            children
                .measure(index, width)
                .map(|size| size.1 as usize)
                .unwrap_or_else(|error| {
                    failed = Some(error);
                    0
                })
        });
        if let Some(error) = failed {
            return Err(error);
        }
        self.top = Some((children.ids()[first], first, skipped));
        let mut top = -(skipped as i64);
        for index in first..count {
            if top >= i64::from(height) {
                break;
            }
            let rows = children.measure(index, width)?.1;
            children.place(
                index,
                (
                    0,
                    top.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
                ),
                width,
            )?;
            top += i64::from(rows);
        }
        Ok(())
    }
    fn focusable(&self) -> bool {
        true
    }
    fn layout(&self) -> Layout {
        Layout {
            overflow: taffy::Point {
                x: taffy::Overflow::Hidden,
                y: taffy::Overflow::Hidden,
            },
            flex_grow: 1.0,
            ..Layout::default()
        }
    }
    /// Scrolling is settled at the next paint, where children can be measured,
    /// so an event that would move past either end is left to bubble.
    fn event(&mut self, event: &Event) -> Response {
        // Scrolling is measured against a painted view.
        if self.height == 0 {
            return Response::IGNORE;
        }
        let page = isize::try_from(self.height).unwrap_or(1);
        let (first, last) = self.ends;
        let by = match event {
            Event::Key(Key::Up, _) => -1,
            Event::Key(Key::Down, _) => 1,
            Event::Key(Key::PageUp, _) => -page,
            Event::Key(Key::PageDown, _) => page,
            Event::Mouse(mouse) if mouse.kind == MouseKind::ScrollUp => -1,
            Event::Mouse(mouse) if mouse.kind == MouseKind::ScrollDown => 1,
            Event::Key(Key::Home, _) if !first => {
                (self.top, self.follow, self.pending) = (None, false, 0);
                return Response::REPAINT;
            }
            Event::Key(Key::End, _) if !last || !self.follow => {
                (self.follow, self.pending) = (true, 0);
                return Response::REPAINT;
            }
            _ => return Response::IGNORE,
        };
        if (by < 0 && first) || (by > 0 && last) {
            return Response::IGNORE;
        }
        // Move from what was painted, not from a tail that has since grown.
        self.follow = false;
        self.pending = self.pending.saturating_add(by);
        Response::REPAINT
    }
}
