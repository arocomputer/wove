//! Questions the terminal answers on stdin: where the cursor is, and what
//! color its background has.
use std::io::{self, Write};

/// Zero-based `(column, row)` of the cursor, in raw mode.
#[cfg(unix)]
pub fn cursor(output: &mut impl Write) -> io::Result<Option<(u16, u16)>> {
    let reply = ask(output, b"\x1b[6n", |reply| reply.ends_with(b"R"))?;
    Ok(parse_cursor(&reply))
}
#[cfg(not(unix))]
pub fn cursor(_: &mut impl Write) -> io::Result<Option<(u16, u16)>> {
    Ok(crossterm::cursor::position().ok())
}

/// The background color. Terminals that do not know the question still answer
/// the device-attributes request sent after it, which ends the wait early.
#[cfg(unix)]
pub fn background(output: &mut impl Write) -> io::Result<Option<(u8, u8, u8)>> {
    let reply = ask(output, b"\x1b]11;?\x1b\\\x1b[c", |reply| {
        reply.ends_with(b"c") && reply.windows(3).any(|w| w == b"\x1b[?")
    })?;
    Ok(parse_background(&reply))
}
#[cfg(not(unix))]
pub fn background(_: &mut impl Write) -> io::Result<Option<(u8, u8, u8)>> {
    Ok(None)
}

/// Write a request and collect stdin until `done` or a short deadline.
#[cfg(unix)]
fn ask(
    output: &mut impl Write,
    request: &[u8],
    done: impl Fn(&[u8]) -> bool,
) -> io::Result<Vec<u8>> {
    use rustix::event::{poll, PollFd, PollFlags, Timespec};
    use std::time::{Duration, Instant};
    output.write_all(request)?;
    output.flush()?;
    let stdin = rustix::stdio::stdin();
    let deadline = Instant::now() + Duration::from_millis(300);
    let mut reply = Vec::new();
    while !done(&reply) && reply.len() < 256 {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        let timeout = Timespec {
            tv_sec: 0,
            tv_nsec: left.subsec_nanos().into(),
        };
        let mut fds = [PollFd::new(&stdin, PollFlags::IN)];
        match poll(&mut fds, Some(&timeout)) {
            Ok(0) => break,
            Ok(_) => {}
            Err(rustix::io::Errno::INTR) => continue,
            Err(error) => return Err(error.into()),
        }
        let mut chunk = [0; 64];
        match rustix::io::read(stdin, &mut chunk) {
            Ok(0) => break,
            Ok(n) => reply.extend_from_slice(&chunk[..n]),
            Err(rustix::io::Errno::INTR | rustix::io::Errno::AGAIN) => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(reply)
}

/// `ESC [ row ; column R`, one-based.
#[cfg_attr(not(unix), allow(dead_code))]
fn parse_cursor(reply: &[u8]) -> Option<(u16, u16)> {
    let reply = std::str::from_utf8(reply).ok()?;
    let body = reply.get(reply.rfind("\x1b[")? + 2..reply.rfind('R')?)?;
    let (row, column) = body.split_once(';')?;
    Some((
        column.parse::<u16>().ok()?.checked_sub(1)?,
        row.parse::<u16>().ok()?.checked_sub(1)?,
    ))
}

/// `ESC ] 11 ; rgb:RRRR/GGGG/BBBB`, with one to four hex digits per channel.
#[cfg_attr(not(unix), allow(dead_code))]
fn parse_background(reply: &[u8]) -> Option<(u8, u8, u8)> {
    let reply = String::from_utf8_lossy(reply);
    let body = &reply[reply.find("]11;rgb:")? + 8..];
    let mut channels = body.split('/').take(3).map(|channel| {
        let digits: String = channel
            .chars()
            .take_while(char::is_ascii_hexdigit)
            .take(4)
            .collect();
        let value = u32::from_str_radix(&digits, 16).ok()?;
        let max = (1u32 << (4 * digits.len() as u32)) - 1;
        Some((value * 255 / max) as u8)
    });
    Some((channels.next()??, channels.next()??, channels.next()??))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies_parse_with_surrounding_noise() {
        assert_eq!(parse_cursor(b"x\x1b[12;40R"), Some((39, 11)));
        assert_eq!(parse_cursor(b"\x1b[?62c"), None);
        assert_eq!(
            parse_background(b"\x1b]11;rgb:ffff/8080/0000\x1b\\\x1b[?62c"),
            Some((255, 128, 0))
        );
        assert_eq!(
            parse_background(b"\x1b]11;rgb:ff/00/7f\x07"),
            Some((255, 0, 127))
        );
        assert_eq!(parse_background(b"\x1b[?62c"), None);
    }
}
