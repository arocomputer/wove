//! An inline session. Finished lines join the terminal's own scrollback while
//! a one-row input stays live beneath them. Enter commits a line, which a
//! worker thread answers the way a chat or an agent would. F2 opens a full-screen
//! history view; Esc returns to the inline session, or quits from that session.
use std::{sync::mpsc, thread, time::Duration};
use wove::{
    elements::{Document, Input, Text},
    terminal::{self, Terminal},
    Event, Id, Key, Options, ScreenMode, Tree,
};

/// Draw the tree at its natural height.
fn draw(terminal: &mut Terminal, tree: &mut Tree) -> Result<(), Box<dyn std::error::Error>> {
    let (width, _) = terminal.size()?;
    let height = tree.height(width)?;
    Ok(terminal.draw(tree.frame(width, height)?)?)
}

/// Show a line above the input, then release it: the terminal keeps the row
/// and the tree forgets it, so the live frame never grows with the session.
fn commit(
    terminal: &mut Terminal,
    tree: &mut Tree,
    input: Id,
    line: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let text = tree.create(Text::new(line))?;
    tree.insert(tree.root(), text, 0)?;
    draw(terminal, tree)?;
    if let Err(error) = terminal.commit(1) {
        if error.kind() != std::io::ErrorKind::WouldBlock {
            return Err(error.into());
        }
        terminal.flush()?;
        terminal.commit(1)?;
    }
    tree.remove(text)?;
    tree.focus(Some(input))?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut tree = Tree::new();
    let input = tree.add(
        tree.root(),
        Input {
            placeholder: "Type a line".into(),
            ..Input::default()
        },
    )?;
    // Leaving the mouse alone keeps the terminal's own text selection.
    let mut terminal = Terminal::with_options(Options {
        screen: ScreenMode::Inline,
        mouse: false,
        ..Options::default()
    })?;
    // Frames write on another thread. Committing history waits only when
    // that writer already has a commit pending, keeping its queue bounded.
    terminal.detach()?;
    let header = "Wove inline · Enter commits a line · Esc quits";
    commit(&mut terminal, &mut tree, input, header)?;
    // The worker wakes the loop, so its answer is drawn without a key press.
    let waker = terminal::waker()?;
    let (ask, questions) = mpsc::channel::<String>();
    let (answer, answers) = mpsc::channel();
    thread::spawn(move || {
        for line in questions {
            thread::sleep(Duration::from_millis(50));
            if answer.send(format!("echo: {line}")).is_err() {
                break;
            }
            waker.wake();
        }
    });
    let mut history = Vec::new();
    let mut review = Tree::new();
    review.add(review.root(), Text::new("Wove history · Esc returns"))?;
    let document = review.add(review.root(), Document::default())?;
    let mut layout = review.layout(document)?.clone();
    layout.flex_grow = 1.0;
    review.set_layout(document, layout)?;
    review.focus(Some(document))?;
    let mut reviewing = false;
    // Keys typed while the session was starting come first.
    let mut typed = terminal.typed_ahead().into_iter();
    loop {
        if !reviewing {
            for line in answers.try_iter() {
                commit(&mut terminal, &mut tree, input, &line)?;
                history.push(line);
            }
        }
        if reviewing {
            let (width, height) = terminal.size()?;
            terminal.draw(review.frame(width, height)?)?;
        } else {
            draw(&mut terminal, &mut tree)?;
        }
        let event = match typed.next() {
            Some(event) => Some(event),
            None => terminal::read()?,
        };
        let Some(event) = event else {
            continue;
        };
        match event {
            Event::Key(Key::Escape, _) if reviewing => {
                reviewing = false;
                terminal.switch(ScreenMode::Inline)?;
            }
            Event::Key(Key::Escape, _) => break,
            Event::Key(Key::Function(2), _) if !reviewing => {
                review.update::<Document>(document, |document| {
                    document.content = history.join("\n")
                })?;
                terminal.switch(ScreenMode::Alternate)?;
                reviewing = true;
            }
            event if reviewing => {
                review.dispatch(event)?;
            }
            Event::Key(Key::Enter, _) => {
                let line = tree.get::<Input>(input)?.editor.text().to_owned();
                tree.update::<Input>(input, |input| input.editor.set(""))?;
                commit(&mut terminal, &mut tree, input, &line)?;
                history.push(line.clone());
                ask.send(line)?;
            }
            event => {
                tree.dispatch(event)?;
            }
        }
    }
    Ok(())
}
