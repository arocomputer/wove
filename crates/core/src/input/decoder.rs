//! Terminal input bytes decoded into portable events, for transports that
//! carry raw bytes instead of a local terminal.
use super::{Button, Event, Key, Modifiers, Mouse, MouseKind};

/// Input the decoder refuses to keep or cannot represent.
#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// A paste or escape sequence grew past its limit without ending.
    Overflow,
    /// The bytes are not UTF-8.
    Encoding,
}
impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Overflow => "terminal input exceeds limit",
            Self::Encoding => "terminal input is not UTF-8",
        })
    }
}
impl std::error::Error for DecodeError {}

const PASTE_LIMIT: usize = 65536;
const SEQUENCE_LIMIT: usize = 64;

/// Decode xterm and kitty key reports, SGR mouse reports, focus changes, and
/// bracketed paste. Input may arrive split anywhere; only an incomplete
/// sequence is retained between calls.
#[derive(Default)]
pub struct Decoder {
    pending: Vec<u8>,
    paste: bool,
}

impl Decoder {
    /// A lone escape byte is waiting: it is either the Escape key or the start
    /// of a sequence. Callers wait briefly, then call `flush_escape`.
    pub fn escape_pending(&self) -> bool {
        !self.paste && self.pending == b"\x1b"
    }

    /// Resolve a waiting escape byte as the Escape key.
    pub fn flush_escape(&mut self) -> Vec<Event> {
        if self.escape_pending() {
            self.pending.clear();
            vec![Key::Escape.into()]
        } else {
            Vec::new()
        }
    }

    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<Event>, DecodeError> {
        let mut events = Vec::new();
        for &byte in bytes {
            self.pending.push(byte);
            if self.pending.len() > PASTE_LIMIT + 6 {
                return Err(DecodeError::Overflow);
            }
            if self.paste {
                if self.pending.ends_with(b"\x1b[201~") {
                    let text = std::str::from_utf8(&self.pending[..self.pending.len() - 6])
                        .map_err(|_| DecodeError::Encoding)?;
                    events.push(Event::Paste(text.into()));
                    self.pending.clear();
                    self.paste = false;
                }
                continue;
            }
            if self.pending == b"\x1b[200~" {
                self.pending.clear();
                self.paste = true;
                continue;
            }
            if self.pending.starts_with(b"\x1b[") || self.pending.starts_with(b"\x1bO") {
                if self.pending.len() > SEQUENCE_LIMIT {
                    return Err(DecodeError::Overflow);
                }
                if self.pending.len() > 2 && (0x40..=0x7e).contains(&byte) {
                    events.extend(sequence(&self.pending));
                    self.pending.clear();
                }
                continue;
            }
            if self.escape_pending() {
                continue;
            }
            let alt = self.pending[0] == 0x1b;
            let text = match std::str::from_utf8(&self.pending[usize::from(alt)..]) {
                Ok(text) => text,
                Err(error) if error.error_len().is_none() => continue,
                Err(_) => return Err(DecodeError::Encoding),
            };
            let mut modifiers = Modifiers {
                alt,
                ..Modifiers::default()
            };
            let key = match text.chars().next() {
                Some('\r' | '\n') => Key::Enter,
                Some('\t') => Key::Tab,
                Some('\x7f' | '\x08') => Key::Backspace,
                Some('\x1b') => Key::Escape,
                Some(c @ '\x01'..='\x1a') => {
                    modifiers.ctrl = true;
                    Key::Char(char::from(c as u8 + b'a' - 1))
                }
                Some('\0') => {
                    modifiers.ctrl = true;
                    Key::Char(' ')
                }
                Some(c) => Key::Char(c),
                None => continue,
            };
            events.push(Event::Key(key, modifiers));
            self.pending.clear();
        }
        Ok(events)
    }
}

/// xterm's modifier parameter: one more than a bit mask. Kitty adds super,
/// hyper, and meta, which all report as `meta`.
fn modifiers(parameter: u32) -> Modifiers {
    let mask = parameter.saturating_sub(1);
    Modifiers {
        shift: mask & 1 != 0,
        alt: mask & 2 != 0,
        ctrl: mask & 4 != 0,
        meta: mask & (8 | 16 | 32) != 0,
    }
}

/// Interpret one complete CSI or SS3 report; unsupported reports are ignored.
fn sequence(bytes: &[u8]) -> Option<Event> {
    let end = *bytes.last()?;
    let body = std::str::from_utf8(&bytes[2..bytes.len() - 1]).ok()?;
    if let Some(body) = body.strip_prefix('<') {
        return mouse(body, end);
    }
    // Each `;` field may carry `:` sub-parameters; an omitted value reads as 1.
    let fields: Vec<Vec<u32>> = body
        .split(';')
        .map(|field| field.split(':').map(|n| n.parse().unwrap_or(1)).collect())
        .collect();
    let value = |field: usize, part: usize| fields.get(field)?.get(part).copied();
    if value(1, 1) == Some(3) {
        return None; // A key release.
    }
    let mut modifiers = modifiers(value(1, 0).unwrap_or(1));
    let key = match end {
        b'A' => Key::Up,
        b'B' => Key::Down,
        b'C' => Key::Right,
        b'D' => Key::Left,
        b'H' => Key::Home,
        b'F' => Key::End,
        b'P'..=b'S' => Key::Function(end - b'P' + 1),
        b'Z' => {
            modifiers.shift = true;
            Key::Tab
        }
        b'I' => return Some(Event::Focus),
        b'O' => return Some(Event::Blur),
        b'u' => match value(0, 0)? {
            9 => Key::Tab,
            13 => Key::Enter,
            27 => Key::Escape,
            127 => Key::Backspace,
            // Lock and modifier keys reported on their own.
            57358..=57363 | 57441..=57454 => return None,
            code => Key::Char(char::from_u32(code).filter(|c| !c.is_control())?),
        },
        b'~' => match value(0, 0)? {
            1 | 7 => Key::Home,
            2 => Key::Insert,
            3 => Key::Delete,
            4 | 8 => Key::End,
            5 => Key::PageUp,
            6 => Key::PageDown,
            n @ (11..=15) => Key::Function((n - 10) as u8),
            n @ (17..=21) => Key::Function((n - 11) as u8),
            n @ (23..=24) => Key::Function((n - 12) as u8),
            _ => return None,
        },
        _ => return None,
    };
    Some(Event::Key(key, modifiers))
}

/// An SGR mouse report: `button;column;row` ending in `M` (press or motion)
/// or `m` (release).
fn mouse(body: &str, end: u8) -> Option<Event> {
    let values: Vec<u16> = body
        .split(';')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    let [code, x, y] = values[..] else {
        return None;
    };
    let button = match code & 3 {
        0 => Some(Button::Left),
        1 => Some(Button::Middle),
        2 => Some(Button::Right),
        _ => None,
    };
    let kind = match (end, code & 64 != 0, code & 32 != 0) {
        (b'M', true, _) => match code & 3 {
            0 => MouseKind::ScrollUp,
            1 => MouseKind::ScrollDown,
            2 => MouseKind::ScrollLeft,
            _ => MouseKind::ScrollRight,
        },
        (b'M', false, true) => button.map_or(MouseKind::Move, MouseKind::Drag),
        (b'M', false, false) => MouseKind::Down(button?),
        (b'm', false, _) => MouseKind::Up(button?),
        _ => return None,
    };
    Some(Event::Mouse(Mouse {
        x: x.checked_sub(1)?,
        y: y.checked_sub(1)?,
        kind,
        modifiers: Modifiers {
            shift: code & 4 != 0,
            alt: code & 8 != 0,
            ctrl: code & 16 != 0,
            meta: false,
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packets_preserve_unicode_keys_and_paste() {
        let mut decoder = Decoder::default();
        let mut events = Vec::new();
        for byte in "é\x1b[1;5D\x1b[200~a\n\x03\x1b[A\x1b[201~".bytes() {
            events.extend(decoder.push(&[byte]).unwrap());
        }
        assert_eq!(
            events,
            vec![
                Key::Char('é').into(),
                Event::Key(
                    Key::Left,
                    Modifiers {
                        ctrl: true,
                        ..Modifiers::default()
                    }
                ),
                Event::Paste("a\n\x03\x1b[A".into())
            ]
        );
    }

    #[test]
    fn escape_waits_for_alt_or_timeout() {
        let mut decoder = Decoder::default();
        assert!(decoder.push(b"\x1b").unwrap().is_empty());
        assert_eq!(decoder.flush_escape(), vec![Key::Escape.into()]);
        assert_eq!(
            decoder.push(b"\x1bx").unwrap(),
            vec![Event::Key(
                Key::Char('x'),
                Modifiers {
                    alt: true,
                    ..Modifiers::default()
                }
            )]
        );
    }

    #[test]
    fn incomplete_input_is_bounded() {
        let mut decoder = Decoder::default();
        decoder.push(b"\x1b[200~").unwrap();
        assert_eq!(
            decoder.push(&vec![b'a'; PASTE_LIMIT + 7]),
            Err(DecodeError::Overflow)
        );
        let mut decoder = Decoder::default();
        assert_eq!(
            decoder.push(format!("\x1b[{}", "1".repeat(64)).as_bytes()),
            Err(DecodeError::Overflow)
        );
    }

    #[test]
    fn kitty_reports_carry_modifiers_a_legacy_terminal_cannot_send() {
        let mut decoder = Decoder::default();
        // Shift+Enter, Super+v, then a release that must not repeat the key.
        let events = decoder.push(b"\x1b[13;2u\x1b[118;9u\x1b[118;9:3u").unwrap();
        assert_eq!(
            events,
            vec![
                Event::Key(
                    Key::Enter,
                    Modifiers {
                        shift: true,
                        ..Modifiers::default()
                    }
                ),
                Event::Key(
                    Key::Char('v'),
                    Modifiers {
                        meta: true,
                        ..Modifiers::default()
                    }
                ),
            ]
        );
    }

    #[test]
    fn mouse_reports_keep_button_drag_and_modifiers() {
        let mut decoder = Decoder::default();
        let events = decoder
            .push(b"\x1b[<2;5;3M\x1b[<34;6;3M\x1b[<2;6;3m\x1b[<35;7;3M\x1b[<80;1;1M")
            .unwrap();
        let kinds: Vec<_> = events
            .iter()
            .map(|event| match event {
                Event::Mouse(mouse) => (mouse.x, mouse.kind, mouse.modifiers.ctrl),
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(
            kinds,
            vec![
                (4, MouseKind::Down(Button::Right), false),
                (5, MouseKind::Drag(Button::Right), false),
                (5, MouseKind::Up(Button::Right), false),
                (6, MouseKind::Move, false),
                (0, MouseKind::ScrollUp, true),
            ]
        );
    }
}
