//! Terminal input bytes decoded into portable events, for transports that
//! carry raw bytes instead of a local terminal.
use super::{Button, Event, Key, Modifiers, Mouse, MouseKind};

/// The most paste text delivered as one event. A longer paste arrives as
/// several `Paste` events in order, so memory stays bounded whatever is pasted.
const PASTE_LIMIT: usize = 65536;
/// No report a terminal sends is longer; anything that is gets dropped.
const SEQUENCE_LIMIT: usize = 64;

/// Decode xterm and kitty key reports, SGR mouse reports, focus changes, and
/// bracketed paste. Input may arrive split anywhere; only an incomplete
/// sequence is retained between calls. Malformed input is dropped rather than
/// reported: a peer can send anything, and none of it should end a session.
#[derive(Default)]
pub struct Decoder {
    pending: Vec<u8>,
    paste: bool,
}

impl Decoder {
    /// The input so far is a whole key on its own and also the start of a
    /// longer report: Escape, Alt with `[` or `O`, or any of those after
    /// another Escape, which some terminals send for Alt with a report.
    /// Callers wait briefly for more bytes, then call `flush_escape`.
    pub fn escape_pending(&self) -> bool {
        !self.paste
            && matches!(
                self.pending.as_slice(),
                b"\x1b" | b"\x1b[" | b"\x1bO" | b"\x1b\x1b" | b"\x1b\x1b[" | b"\x1b\x1bO"
            )
    }

    /// Resolve the waiting bytes as the keys they are on their own: two
    /// Escapes are Alt+Escape, and an Escape before Alt with `[` or `O` is a
    /// separate key.
    pub fn flush_escape(&mut self) -> Vec<Event> {
        if !self.escape_pending() {
            return Vec::new();
        }
        let alt = Modifiers {
            alt: true,
            ..Modifiers::default()
        };
        let events = match self.pending[..] {
            [_] => vec![Key::Escape.into()],
            [_, 0x1b] => vec![Event::key(Key::Escape, alt)],
            [_, byte] => vec![Event::key(Key::Char(char::from(byte)), alt)],
            [.., byte] => vec![
                Key::Escape.into(),
                Event::key(Key::Char(char::from(byte)), alt),
            ],
            [] => Vec::new(),
        };
        self.pending.clear();
        events
    }

    /// Decode the events these bytes complete. A byte is text being pasted,
    /// the start of a paste, an Escape that begins new input, part of a
    /// report, or part of a key, in that order.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<Event> {
        let mut events = Vec::new();
        for &byte in bytes {
            self.pending.push(byte);
            if self.paste {
                self.paste_byte(&mut events);
                continue;
            }
            if self.paste_starts(&mut events) {
                continue;
            }
            if self.escape_restarts(byte, &mut events) || self.report_byte(byte, &mut events) {
                continue;
            }
            self.key(&mut events);
        }
        events
    }

    /// A paste begins, possibly after an Escape that is then a key of its own.
    fn paste_starts(&mut self, events: &mut Vec<Event>) -> bool {
        let escaped = self.pending.starts_with(b"\x1b\x1b");
        if &self.pending[usize::from(escaped)..] != b"\x1b[200~" {
            return false;
        }
        if escaped {
            events.push(Key::Escape.into());
        }
        self.pending.clear();
        self.paste = true;
        true
    }

    /// Text inside a paste: delivered at the terminator, or in bounded
    /// pieces when a paste is longer than the limit.
    fn paste_byte(&mut self, events: &mut Vec<Event>) {
        if self.pending.ends_with(b"\x1b[201~") {
            let text = &self.pending[..self.pending.len() - 6];
            events.push(Event::Paste(String::from_utf8_lossy(text).into()));
            self.pending.clear();
            self.paste = false;
        } else if self.pending.len() > PASTE_LIMIT {
            // Deliver what has arrived, keeping enough to recognize a
            // terminator split across the cut, and cut between characters.
            // A character is at most four bytes, so three steps back
            // reach its start; bytes that are not text stop there too.
            let mut cut = self.pending.len() - 5;
            let floor = cut - 3;
            while cut > floor && self.pending[cut] & 0xc0 == 0x80 {
                cut -= 1;
            }
            let text: Vec<u8> = self.pending.drain(..cut).collect();
            events.push(Event::Paste(String::from_utf8_lossy(&text).into()));
        }
    }

    /// Escape never occurs inside a report or a character, so it begins the
    /// next input, except that a second Escape may introduce Alt. Returns
    /// whether the byte was such a restart.
    fn escape_restarts(&mut self, byte: u8, events: &mut Vec<Event>) -> bool {
        if byte != 0x1b || self.pending.len() <= 1 {
            return false;
        }
        let before = &self.pending[..self.pending.len() - 1];
        if before == b"\x1b\x1b" {
            events.push(Event::key(
                Key::Escape,
                Modifiers {
                    alt: true,
                    ..Modifiers::default()
                },
            ));
        }
        if before != b"\x1b" {
            self.pending.clear();
            self.pending.push(byte);
        }
        true
    }

    /// A CSI or SS3 report in progress, with an Escape before it adding Alt.
    /// Returns whether the byte belongs to one, complete or not.
    fn report_byte(&mut self, byte: u8, events: &mut Vec<Event>) -> bool {
        let alt = self.pending.starts_with(b"\x1b\x1b");
        let report = &self.pending[usize::from(alt)..];
        if !report.starts_with(b"\x1b[") && !report.starts_with(b"\x1bO") {
            return false;
        }
        // The Linux console's F1 to F5 have a second `[` before the final byte.
        let bracket = report == b"\x1b[[";
        if self.pending.len() > SEQUENCE_LIMIT {
            self.pending.clear();
        } else if report.len() > 2 && (0x40..=0x7e).contains(&byte) && !bracket {
            let mut event = sequence(report);
            if let Some(Event::Key(_, modifiers)) = &mut event {
                modifiers.alt |= alt;
            }
            events.extend(event);
            self.pending.clear();
        }
        true
    }

    /// A key typed as text, possibly after an Escape that means Alt. Nothing
    /// happens until the character is complete.
    fn key(&mut self, events: &mut Vec<Event>) {
        if self.pending == b"\x1b\x1b" {
            return;
        }
        if self.pending.starts_with(b"\x1b\x1b") {
            // Escape followed by an Alt key.
            events.push(Key::Escape.into());
            self.pending.remove(0);
        }
        let alt = self.pending[0] == 0x1b;
        let text = match std::str::from_utf8(&self.pending[usize::from(alt)..]) {
            Ok(text) => text,
            Err(error) if error.error_len().is_none() => return,
            Err(_) => {
                self.pending.clear();
                return;
            }
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
            Some(c @ '\x1c'..='\x1f') => {
                modifiers.ctrl = true;
                Key::Char(['\\', ']', '^', '_'][c as usize - 0x1c])
            }
            // The C1 range and the rest of C0 name nothing a key sends.
            Some(c) if c.is_control() => {
                self.pending.clear();
                return;
            }
            Some(c) => Key::Char(c),
            None => return,
        };
        events.push(Event::key(key, modifiers));
        self.pending.clear();
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
    if body == "[" {
        return matches!(end, b'A'..=b'E').then(|| Key::Function(end - b'A' + 1).into());
    }
    // Empty sub-parameters stay absent; only modifier fields default to 1.
    let fields: Vec<Vec<Option<u32>>> = body
        .split(';')
        .map(|field| field.split(':').map(|n| n.parse().ok()).collect())
        .collect();
    let value = |field: usize, part: usize| fields.get(field)?.get(part).copied().flatten();
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
        b'I' => return Some(Event::WindowFocus(true)),
        b'O' => return Some(Event::WindowFocus(false)),
        b'u' => kitty_key(value(0, 0)?, value(0, 1), modifiers.shift)?,
        b'~' => tilde_key(value(0, 0)?)?,
        _ => return None,
    };
    Some(Event::key(key, modifiers))
}

/// A kitty keyboard report: the key's codepoint, and the codepoint Shift
/// makes of it on the terminal's layout, which is more accurate than
/// uppercasing the base key.
fn kitty_key(code: u32, shifted: Option<u32>, shift: bool) -> Option<Key> {
    Some(match code {
        9 => Key::Tab,
        13 => Key::Enter,
        27 => Key::Escape,
        127 => Key::Backspace,
        // Kitty reports keys without text as private-use code points. The
        // keypad has ordinary meanings; locks, modifiers, media keys, and
        // the rest must never reach an application as characters.
        code @ 57399..=57408 => Key::Char(char::from(b'0' + (code - 57399) as u8)),
        57409 => Key::Char('.'),
        57410 => Key::Char('/'),
        57411 => Key::Char('*'),
        57412 => Key::Char('-'),
        57413 => Key::Char('+'),
        57414 => Key::Enter,
        57415 => Key::Char('='),
        57417 => Key::Left,
        57418 => Key::Right,
        57419 => Key::Up,
        57420 => Key::Down,
        57421 => Key::PageUp,
        57422 => Key::PageDown,
        57423 => Key::Home,
        57424 => Key::End,
        57425 => Key::Insert,
        57426 => Key::Delete,
        0xe000..=0xf8ff => return None,
        code => {
            let code = if shift { shifted.unwrap_or(code) } else { code };
            let c = char::from_u32(code)
                .filter(|c| !c.is_control() && !(0xe000..=0xf8ff).contains(&code))?;
            Key::Char(c)
        }
    })
}

/// A `~` report: the xterm numbers for editing and function keys.
fn tilde_key(number: u32) -> Option<Key> {
    Some(match number {
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
    })
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
    // Buttons past the third set a high bit; they are not a left click.
    if code & 128 != 0 {
        return None;
    }
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
            events.extend(decoder.push(&[byte]));
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
        assert!(decoder.push(b"\x1b").is_empty());
        assert_eq!(decoder.flush_escape(), vec![Key::Escape.into()]);
        assert_eq!(
            decoder.push(b"\x1bx"),
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
    fn a_huge_paste_arrives_in_order_and_malformed_input_is_dropped() {
        let mut decoder = Decoder::default();
        let body = "é".repeat(PASTE_LIMIT);
        let mut events = decoder.push(b"\x1b[200~");
        events.extend(decoder.push(body.as_bytes()));
        events.extend(decoder.push(b"\x1b[201~x"));
        let pasted: String = events
            .iter()
            .filter_map(|event| match event {
                Event::Paste(text) => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(pasted, body, "chunks never split a character");
        assert!(events.len() > 3);
        assert_eq!(events.last(), Some(&Key::Char('x').into()));
        // A paste of bytes that are not text is cut without walking off its start.
        let mut decoder = Decoder::default();
        decoder.push(b"\x1b[200~");
        let events = decoder.push(&vec![0x80; PASTE_LIMIT + 16]);
        assert!(matches!(events[..], [Event::Paste(_), ..]));
        // A report for a button past the third is not a left click.
        assert!(Decoder::default().push(b"\x1b[<128;1;1M").is_empty());
        // An endless report and invalid UTF-8 are dropped; typing continues.
        let mut decoder = Decoder::default();
        let mut events = decoder.push(format!("\x1b[{}", "1".repeat(80)).as_bytes());
        events.extend(decoder.push(b"\xff"));
        events.extend(decoder.push(b"ok"));
        assert_eq!(
            events[events.len() - 2..],
            [Key::Char('o').into(), Key::Char('k').into()]
        );
    }

    #[test]
    fn alt_with_a_bracket_is_a_key_once_nothing_follows() {
        let mut decoder = Decoder::default();
        assert!(decoder.push(b"\x1b[").is_empty());
        assert!(decoder.escape_pending());
        let alt = Modifiers {
            alt: true,
            ..Modifiers::default()
        };
        assert_eq!(decoder.flush_escape(), [Event::Key(Key::Char('['), alt)]);
        assert_eq!(decoder.push(b"a"), [Key::Char('a').into()]);
        // Bytes that do follow in time still make the report they belong to.
        assert_eq!(decoder.push(b"\x1b[A"), [Key::Up.into()]);
    }

    #[test]
    fn an_escape_before_a_report_is_alt_and_nothing_leaks_as_typing() {
        let alt = Modifiers {
            alt: true,
            ..Modifiers::default()
        };
        let mut decoder = Decoder::default();
        // rxvt and macOS send Alt+Up as Escape before the report.
        assert_eq!(decoder.push(b"\x1b\x1b[A"), [Event::key(Key::Up, alt)]);
        assert_eq!(decoder.push(b"\x1b\x1bOB"), [Event::key(Key::Down, alt)]);
        // Escape then Alt+x, and Alt+Escape once nothing follows.
        assert_eq!(
            decoder.push(b"\x1b\x1bx"),
            [Key::Escape.into(), Event::key(Key::Char('x'), alt)]
        );
        assert!(decoder.push(b"\x1b\x1b").is_empty());
        assert_eq!(decoder.flush_escape(), [Event::key(Key::Escape, alt)]);
        // An escape inside a report abandons it and starts the next one.
        assert_eq!(decoder.push(b"\x1b[1;\x1b[B"), [Key::Down.into()]);
        // The Linux console reports F1 to F5 with a second bracket.
        assert_eq!(
            decoder.push(b"\x1b[[A\x1b[[E"),
            [Key::Function(1).into(), Key::Function(5).into()]
        );
    }

    #[test]
    fn kitty_reports_carry_modifiers_a_legacy_terminal_cannot_send() {
        let mut decoder = Decoder::default();
        // Shift+Enter, Super+v, then a release that must not repeat the key.
        let mut events = decoder.push(b"\x1b[13;2u\x1b[118;9u\x1b[118;9:3u");
        // Keypad Enter is Enter; a media key is not a character.
        assert_eq!(decoder.push(b"\x1b[57414u\x1b[57430u"), [Key::Enter.into()]);
        events.truncate(2);
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
    fn kitty_uses_the_shifted_codepoint_without_mistaking_an_omitted_one_for_text() {
        let shift = Modifiers {
            shift: true,
            ..Modifiers::default()
        };
        for (bytes, expected) in [
            (
                b"\x1b[49:33;2u".as_slice(),
                Event::key(Key::Char('!'), shift),
            ),
            (b"\x1b[97:65;2u", Key::Char('A').into()),
            (b"\x1b[49::49;2u", Event::key(Key::Char('1'), shift)),
            (b"\x1b[49:33u", Key::Char('1').into()),
        ] {
            assert_eq!(Decoder::default().push(bytes), [expected]);
        }
        for bytes in [
            b"\x1b[49:27;2u".as_slice(),
            b"\x1b[49:57430;2u",
            b"\x1b[49:1114112;2u",
        ] {
            assert!(Decoder::default().push(bytes).is_empty());
        }
    }

    #[test]
    fn a_paste_after_an_escape_is_a_paste_and_control_characters_are_keys_or_nothing() {
        let mut decoder = Decoder::default();
        assert!(decoder.push(b"\x1b").is_empty());
        let events = decoder.push(b"\x1b[200~text\n\x1b[201~");
        assert_eq!(events, [Key::Escape.into(), Event::Paste("text\n".into())]);
        let ctrl = Modifiers {
            ctrl: true,
            ..Modifiers::default()
        };
        assert_eq!(
            decoder.push(b"\x1c\x1f"),
            [
                Event::key(Key::Char('\\'), ctrl),
                Event::key(Key::Char('_'), ctrl)
            ]
        );
        // A C1 control is not a character anyone typed.
        assert!(decoder.push("\u{85}".as_bytes()).is_empty());
        assert_eq!(decoder.push(b"a"), [Key::Char('a').into()]);
    }

    #[test]
    fn mouse_reports_keep_button_drag_and_modifiers() {
        let mut decoder = Decoder::default();
        let events =
            decoder.push(b"\x1b[<2;5;3M\x1b[<34;6;3M\x1b[<2;6;3m\x1b[<35;7;3M\x1b[<80;1;1M");
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
