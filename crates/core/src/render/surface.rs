//! Requests to the terminal around the frame: the clipboard, the window
//! title, the bell, and taskbar progress. Each is a pure byte builder, so a
//! transport such as SSH can send the same bytes a local session does.
//!
//! Caller text is never passed through raw: the clipboard is base64-encoded,
//! and a title with control characters is refused.
use super::pen::embeddable;

/// The bytes that ask a terminal to put `text` on the system clipboard
/// (OSC 52). Inside tmux the request is wrapped so tmux passes it on; pass
/// whether the `TMUX` environment variable is set. Terminals that do not
/// support the request, or have it turned off, ignore it.
///
/// Nothing reads the clipboard back. Pasted text arrives as `Event::Paste`
/// when `Options::paste` is on; any other read belongs to the application.
pub fn clipboard(text: &str, tmux: bool) -> Vec<u8> {
    let mut request = b"\x1b]52;c;".to_vec();
    request.extend(base64(text.as_bytes()));
    request.extend_from_slice(b"\x1b\\");
    passthrough(request, tmux)
}

/// The bytes that set the window and tab title (OSC 0), or `None` when
/// `text` holds a control character or is longer than 2048 bytes. tmux keeps
/// the title as the pane's own, and shows it in the outer terminal when its
/// `set-titles` option is on, so the request is never wrapped.
pub fn title(text: &str) -> Option<Vec<u8>> {
    embeddable(text).then(|| [b"\x1b]0;", text.as_bytes(), b"\x1b\\"].concat())
}

/// Save the current title on the terminal's title stack (xterm window
/// operations), so that `POP_TITLE` can put it back. Terminals without the
/// stack ignore both.
#[cfg_attr(not(feature = "terminal"), allow(dead_code))]
pub(crate) const PUSH_TITLE: &[u8] = b"\x1b[22;0t";
/// Restore the title saved by `PUSH_TITLE`.
#[cfg_attr(not(feature = "terminal"), allow(dead_code))]
pub(crate) const POP_TITLE: &[u8] = b"\x1b[23;0t";

/// The byte that rings the terminal bell (BEL). What it does, a sound, a
/// flash, or a mark on the tab, is the terminal's setting.
pub const BELL: &[u8] = b"\x07";

/// Taskbar or tab progress, as terminals that follow ConEmu's OSC 9;4 show it.
/// Percentages above 100 are shown as 100.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
    /// Remove the indicator.
    Clear,
    /// Ordinary progress, in percent.
    Normal(u8),
    /// Progress that has failed, usually shown in red.
    Error(u8),
    /// Busy for an unknown time.
    Indeterminate,
    /// Progress that is waiting or warns, usually shown in yellow.
    Paused(u8),
}

/// The bytes that show `progress` (OSC 9;4). Terminals without the request
/// ignore it, except that a few older ones read any OSC 9 as a desktop
/// notification. Inside tmux it is wrapped so tmux passes it on, as for
/// `clipboard`.
pub fn progress(progress: Progress, tmux: bool) -> Vec<u8> {
    let (state, percent) = match progress {
        Progress::Clear => (0, 0),
        Progress::Normal(percent) => (1, percent),
        Progress::Error(percent) => (2, percent),
        Progress::Indeterminate => (3, 0),
        Progress::Paused(percent) => (4, percent),
    };
    let request = format!("\x1b]9;4;{state};{}\x1b\\", percent.min(100));
    passthrough(request.into_bytes(), tmux)
}

/// Wrap a request in tmux's passthrough when `tmux` is set, doubling every
/// escape byte inside the wrapper.
fn passthrough(request: Vec<u8>, tmux: bool) -> Vec<u8> {
    if !tmux {
        return request;
    }
    let mut wrapped = b"\x1bPtmux;".to_vec();
    for byte in request {
        if byte == 0x1b {
            wrapped.push(0x1b);
        }
        wrapped.push(byte);
    }
    wrapped.extend_from_slice(b"\x1b\\");
    wrapped
}

fn base64(bytes: &[u8]) -> Vec<u8> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let group = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let bits = u32::from(group[0]) << 16 | u32::from(group[1]) << 8 | u32::from(group[2]);
        for position in 0..4 {
            out.push(if position <= chunk.len() {
                ALPHABET[(bits >> (18 - 6 * position) & 63) as usize]
            } else {
                b'='
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_carry_padded_base64_and_tmux_wraps_them() {
        assert_eq!(clipboard("hi", false), b"\x1b]52;c;aGk=\x1b\\");
        assert_eq!(clipboard("hey", false), b"\x1b]52;c;aGV5\x1b\\");
        assert_eq!(
            clipboard("h", true),
            b"\x1bPtmux;\x1b\x1b]52;c;aA==\x1b\x1b\\\x1b\\"
        );
    }

    #[test]
    fn a_title_that_could_end_its_sequence_is_refused() {
        assert_eq!(
            title("build · 3/5").unwrap(),
            "\x1b]0;build · 3/5\x1b\\".as_bytes()
        );
        for hostile in ["a\x1b\\b", "a\x07b", "a\nb", "a\u{9c}b"] {
            assert_eq!(title(hostile), None, "{hostile:?}");
        }
        assert!(title(&"x".repeat(2048)).is_some());
        assert_eq!(title(&"x".repeat(2049)), None);
    }

    #[test]
    fn progress_states_follow_osc_9_4_and_cap_at_one_hundred() {
        assert_eq!(
            progress(Progress::Normal(40), false),
            b"\x1b]9;4;1;40\x1b\\"
        );
        assert_eq!(
            progress(Progress::Error(250), false),
            b"\x1b]9;4;2;100\x1b\\"
        );
        assert_eq!(
            progress(Progress::Indeterminate, false),
            b"\x1b]9;4;3;0\x1b\\"
        );
        assert_eq!(progress(Progress::Paused(7), false), b"\x1b]9;4;4;7\x1b\\");
        assert_eq!(progress(Progress::Clear, false), b"\x1b]9;4;0;0\x1b\\");
        assert_eq!(
            progress(Progress::Clear, true),
            b"\x1bPtmux;\x1b\x1b]9;4;0;0\x1b\x1b\\\x1b\\"
        );
    }
}
