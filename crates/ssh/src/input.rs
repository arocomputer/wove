use wove::{Event, Key, Modifiers, Mouse, MouseKind};

use crate::Error;

/// Decode terminal input across SSH packets, retaining only incomplete sequences.
#[derive(Default)]
pub(crate) struct Decoder {
    pending: Vec<u8>,
    paste: bool,
}

impl Decoder {
    pub fn escape_pending(&self) -> bool {
        !self.paste && self.pending == b"\x1b"
    }

    pub fn flush_escape(&mut self) -> Vec<Event> {
        if self.escape_pending() {
            self.pending.clear();
            vec![Key::Escape.into()]
        } else {
            Vec::new()
        }
    }

    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<Event>, Error> {
        let mut events = Vec::new();
        for &byte in bytes {
            self.pending.push(byte);
            if self.pending.len() > 65542 {
                return Err("terminal input exceeds limit".into());
            }
            if self.paste {
                if self.pending.ends_with(b"\x1b[201~") {
                    let text = std::str::from_utf8(&self.pending[..self.pending.len() - 6])?;
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
                if self.pending.len() > 64 {
                    return Err("terminal sequence exceeds limit".into());
                }
                if self.pending.len() > 2 && (0x40..=0x7e).contains(&byte) {
                    if let Some(event) = sequence(&self.pending) {
                        events.push(event);
                    }
                    self.pending.clear();
                }
                continue;
            }
            if self.escape_pending() {
                continue;
            }
            let alt = self.pending[0] == 0x1b;
            let text = if alt {
                &self.pending[1..]
            } else {
                &self.pending[..]
            };
            let text = match std::str::from_utf8(text) {
                Ok(text) => text,
                Err(error) if error.error_len().is_none() => continue,
                Err(error) => return Err(error.into()),
            };
            let mut modifiers = Modifiers {
                alt,
                ..Modifiers::default()
            };
            let key = match text.chars().next().unwrap() {
                '\r' | '\n' => Key::Enter,
                '\t' => Key::Tab,
                '\x7f' | '\x08' => Key::Backspace,
                '\x1b' => Key::Escape,
                c @ '\x01'..='\x1a' => {
                    modifiers.ctrl = true;
                    Key::Char(char::from(c as u8 + b'a' - 1))
                }
                '\0' => {
                    modifiers.ctrl = true;
                    Key::Char(' ')
                }
                c => Key::Char(c),
            };
            events.push(Event::Key(key, modifiers));
            self.pending.clear();
        }
        Ok(events)
    }
}

/// Interpret common xterm key and SGR mouse reports; ignore unsupported reports.
fn sequence(bytes: &[u8]) -> Option<Event> {
    let end = *bytes.last()?;
    let body = std::str::from_utf8(&bytes[2..bytes.len() - 1]).ok()?;
    if let Some(body) = body.strip_prefix('<') {
        let values: Vec<u16> = body
            .split(';')
            .map(str::parse)
            .collect::<Result<_, _>>()
            .ok()?;
        if values.len() != 3 || !matches!(end, b'M' | b'm') {
            return None;
        }
        let kind = if values[0] & 64 != 0 {
            if values[0] & 1 == 0 {
                MouseKind::ScrollUp
            } else {
                MouseKind::ScrollDown
            }
        } else if end == b'm' {
            MouseKind::Up
        } else if values[0] & 32 != 0 {
            MouseKind::Move
        } else {
            MouseKind::Down
        };
        return Some(Event::Mouse(Mouse {
            x: values[1].checked_sub(1)?,
            y: values[2].checked_sub(1)?,
            kind,
        }));
    }
    let values: Vec<u16> = if body.is_empty() {
        vec![]
    } else {
        body.split(';')
            .map(str::parse)
            .collect::<Result<_, _>>()
            .ok()?
    };
    let mask = values.get(1).copied().unwrap_or(1).checked_sub(1)?;
    let mut modifiers = Modifiers {
        shift: mask & 1 != 0,
        alt: mask & 2 != 0,
        ctrl: mask & 4 != 0,
    };
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
        b'~' => match values.first()? {
            1 | 7 => Key::Home,
            2 => Key::Insert,
            3 => Key::Delete,
            4 | 8 => Key::End,
            5 => Key::PageUp,
            6 => Key::PageDown,
            15 => Key::Function(5),
            17..=21 => Key::Function((*values.first()? - 11) as u8),
            23..=24 => Key::Function((*values.first()? - 12) as u8),
            _ => return None,
        },
        _ => return None,
    };
    Some(Event::Key(key, modifiers))
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
        assert!(decoder.push(&vec![b'a'; 65543]).is_err());
        let mut decoder = Decoder::default();
        assert!(decoder
            .push(format!("\x1b[{}", "1".repeat(64)).as_bytes())
            .is_err());
    }
}
