//! Minimal application: application state, one view, and keyboard updates.
use weft::{Application, Buffer, Event, KeyCode, Style, Text, Widget};

#[derive(Default)]
struct Counter {
    value: i64,
}

impl Application for Counter {
    fn view(&self, frame: &mut Buffer) {
        let text = format!(
            "weft counter\n\n{}\n\n+ / - change the count · q quits",
            self.value
        );
        Text {
            content: &text,
            style: Style::default(),
        }
        .render(frame.area().inset(1), frame);
    }
    fn update(&mut self, event: Event) -> bool {
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char('+') | KeyCode::Up => self.value = self.value.saturating_add(1),
                KeyCode::Char('-') | KeyCode::Down => self.value = self.value.saturating_sub(1),
                KeyCode::Char('q') | KeyCode::Esc => return false,
                _ => {}
            }
        }
        true
    }
}

fn main() -> std::io::Result<()> {
    weft::run(&mut Counter::default())
}
