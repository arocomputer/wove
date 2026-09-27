//! Append-only log viewing with logical selection, follow-tail, and a worker wakeup.
use std::{sync::mpsc, thread, time::Duration};
use wove::{
    elements::{Document, Text},
    terminal::{self, Terminal},
    text::Wrap,
    Event, Key, Tree,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut tree = Tree::new();
    tree.add(
        tree.root(),
        Text::new("Wove logs · Space streams · c copies · Esc quits"),
    )?;
    let mut document = Document::new(
        (0..100)
            .map(|i| format!("record {i:04}  synthetic request completed\n"))
            .collect::<String>(),
    );
    document.wrap = Wrap::Word;
    document.follow = true;
    let log = tree.add(tree.root(), document)?;
    let mut layout = tree.layout(log)?.clone();
    layout.flex_grow = 1.0;
    tree.set_layout(log, layout)?;
    tree.focus(Some(log))?;
    let mut terminal = Terminal::new()?;
    terminal.detach()?;
    let waker = terminal::waker()?;
    let (send, receive) = mpsc::channel();
    thread::spawn(move || {
        for i in 100.. {
            thread::sleep(Duration::from_millis(200));
            if send
                .send(format!("record {i:04}  synthetic request completed\n"))
                .is_err()
            {
                break;
            }
            waker.wake();
        }
    });
    let mut streaming = false;
    let mut typed = terminal.typed_ahead().into_iter();
    loop {
        for line in receive.try_iter() {
            if streaming {
                tree.repaint::<Document>(log, |document| document.content.push_str(&line))?;
            }
        }
        let (width, height) = terminal.size()?;
        terminal.draw(tree.frame(width, height)?)?;
        let event = match typed.next() {
            Some(event) => Some(event),
            None => terminal::read()?,
        };
        let Some(event) = event else {
            continue;
        };
        match event {
            Event::Key(Key::Escape, _) => break,
            Event::Key(Key::Char(' '), _) => streaming = !streaming,
            Event::Key(Key::Char('c'), _) => {
                if let Some(text) = tree.selected_text() {
                    terminal.copy(&text)?;
                }
            }
            Event::Resize(..) => {
                terminal.invalidate();
                tree.dispatch(event)?;
            }
            event => {
                tree.dispatch(event)?;
            }
        }
    }
    Ok(())
}
