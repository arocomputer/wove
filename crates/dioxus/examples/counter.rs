use dioxus::prelude::*;
use wove::{
    terminal::{self, Terminal},
    Event, Key,
};
use wove_dioxus::{elements as dioxus_elements, View};
fn app() -> Element {
    let mut count = use_signal(|| 0);
    let mut name = use_signal(String::new);
    rsx! { view { direction:"column", onkey: move |event| {
        if let Event::Key(Key::Char('+'),_) = *event.data { count += 1; event.prevent_default(); }
        if let Event::Key(Key::Char('-'),_) = *event.data { count -= 1; event.prevent_default(); }
    },
        text { content:"wove · Dioxus counter" }
        text { content:"Count: {count}" }
        text { content:"+ / - change · Tab focus · Esc quit" }
        input { value:"{name}", placeholder:"Type here", oninput: move |event| name.set(event.data.to_string()) }
        text { content:"Name: {name}" }
    } }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut view = View::new(VirtualDom::new(app))?;
    view.focus_next(false)?;
    let mut terminal = Terminal::new()?;
    loop {
        let (w, h) = terminal.size()?;
        terminal.draw(view.frame(w, h)?)?;
        let Some(event) = terminal::read()? else {
            continue;
        };
        if matches!(
            event,
            Event::Key(Key::Escape, _)
                | Event::Key(Key::Char('c'), wove::Modifiers { ctrl: true, .. })
        ) {
            break;
        }
        view.send(event)?;
    }
    Ok(())
}
