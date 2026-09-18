//! A two-pane catalog demonstrates selection, nested layouts, and composition.
use weft::{
    Application, Axis, Border, Buffer, Color, Constraint, Event, KeyCode, Layout, List, Style,
    Text, Widget,
};

const ITEMS: &[&str] = &["Observatory", "Library", "Workshop", "Garden"];
const DETAILS: &[&str] = &[
    "Observatory\n\nTrack the night sky.\n\nNext session: 21:00\nEquipment: telescope, notebook",
    "Library\n\nBrowse a shared collection.\n\nSections: fiction, science, history",
    "Workshop\n\nMake something useful.\n\nTools: lathe, drill, workbench",
    "Garden\n\nPlan the next planting.\n\nBeds: herbs, vegetables, flowers",
];

#[derive(Default)]
struct Explorer {
    selected: usize,
}

impl Application for Explorer {
    fn view(&self, frame: &mut Buffer) {
        let base = Style::default();
        let accent = Style {
            foreground: Color::Indexed(6),
            bold: true,
            ..base
        };
        let rows = Layout {
            axis: Axis::Vertical,
            tracks: &[
                Constraint::Fixed(1),
                Constraint::Fill(1),
                Constraint::Fixed(1),
            ],
            gap: 1,
        }
        .split(frame.area().inset(1));
        Text {
            content: "weft / places",
            style: accent,
        }
        .render(rows[0], frame);
        let columns = Layout {
            axis: Axis::Horizontal,
            tracks: &[Constraint::Fill(1), Constraint::Fill(2)],
            gap: 1,
        }
        .split(rows[1]);
        Border {
            child: List {
                items: ITEMS,
                selected: Some(self.selected),
                offset: self.selected.saturating_sub(
                    usize::from(columns[0].height.saturating_sub(2)).saturating_sub(1),
                ),
                style: base,
                selected_style: accent,
            },
            style: base,
        }
        .render(columns[0], frame);
        Border {
            child: Text {
                content: DETAILS[self.selected],
                style: base,
            },
            style: base,
        }
        .render(columns[1], frame);
        Text {
            content: "↑ / ↓ select · q quit",
            style: base,
        }
        .render(rows[2], frame);
    }
    fn update(&mut self, event: Event) -> bool {
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => {
                    self.selected = (self.selected + 1).min(ITEMS.len() - 1)
                }
                KeyCode::Char('q') | KeyCode::Esc => return false,
                _ => {}
            }
        }
        true
    }
}

fn main() -> std::io::Result<()> {
    weft::run(&mut Explorer::default())
}
