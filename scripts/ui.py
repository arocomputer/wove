"""Exercise input, resize, and terminal restoration through real Unix PTYs."""
import errno
import fcntl
import os
from pathlib import Path
import pty
import select
import signal
import struct
import subprocess
import sys
import termios
import time
import pyte

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "artifacts/ui"
OUT.mkdir(parents=True, exist_ok=True)


def scenario(name, steps, fullscreen=True):
    """Run an example on a PTY and retain visible frames and terminal bytes.

    An inline example stays on the main screen; its rows must stay in order,
    and still be in order after a resize repaints the rows it has released.
    """
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
    original = termios.tcgetattr(slave)
    screen = pyte.Screen(80, 24)
    stream = pyte.ByteStream(screen)
    raw = bytearray()
    cursor_answered = False
    attributes_answered = False
    binary = ROOT / "target/debug" / name if name == "editor" else ROOT / "target/debug/examples" / name
    process = subprocess.Popen([str(binary)],
                               stdin=slave, stdout=slave, stderr=slave,
                               start_new_session=True, env={**os.environ, "TERM": "xterm-256color"})

    def read():
        """Record and display one available chunk; False once the child side closes."""
        try:
            chunk = os.read(master, 65536)
        except OSError as error:
            if error.errno == errno.EIO:
                return False
            raise
        raw.extend(chunk)
        stream.feed(chunk)
        return bool(chunk)

    def receive(expected, after=0):
        """Drain output until a complete frame begun at or after `after` shows `expected`.

        A frame is complete once the last synchronized-update end follows the last
        begin, so checks and snapshots never observe a half-painted screen.
        """
        nonlocal cursor_answered, attributes_answered
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            if select.select([master], [], [], 0.05)[0]:
                if not read():
                    break
                # Requests can cross reads. The first step arrives during the
                # probe, so both library runners must preserve typed-ahead input.
                reply = bytearray()
                if not cursor_answered and b"\x1b[6n" in raw:
                    cursor_answered = True
                    reply.extend(f"\x1b[{screen.cursor.y + 1};{screen.cursor.x + 1}R".encode())
                if not attributes_answered and b"\x1b[c" in raw:
                    attributes_answered = True
                    reply.extend(steps[0][0])
                    reply.extend(b"\x1b[?62c")
                if reply:
                    os.write(master, reply)
            begin = raw.rfind(b"\x1b[?2026h")
            complete = begin >= after and raw.rfind(b"\x1b[?2026l") > begin
            if complete and expected in "\n".join(screen.display):
                return
            if process.poll() is not None:
                break
        raise AssertionError(f"{name}: missing {expected!r}\n" + "\n".join(screen.display))

    def finish(fullscreen):
        """Quit with Escape and check that the terminal is as it was found."""
        os.write(master, b"\x1b")
        # Keep draining while waiting so cleanup output cannot fill the PTY buffer.
        deadline = time.monotonic() + 10
        while process.poll() is None:
            assert time.monotonic() < deadline, f"{name}: did not exit"
            if select.select([master], [], [], 0.05)[0]:
                read()
        assert process.returncode == 0
        while select.select([master], [], [], 0.05)[0] and read():
            pass
        restored = termios.tcgetattr(slave)
        assert restored == original, f"{name}: terminal attributes were not restored"
        assert (b"\x1b[?1049l" in raw) == fullscreen, "alternate screen use is wrong"
        assert b"\x1b[?25h" in raw, "cursor was not restored"
        assert b"\x1b[?2026h" in raw, "frames were not synchronized"
        print(f"{name}: input and terminal restoration passed")

    def check_border():
        if name == "gallery":
            for row in screen.display[3:-2]:
                assert row[18] == "│" and row[-1] == "│", f"clipped panel border: {row!r}"

    try:
        receive("Wove")
        for index, (keys, expected) in enumerate(steps):
            if index > 0:
                os.write(master, keys)
            receive(expected)
            check_border()
            (OUT / f"{name}-{index}.txt").write_text("\n".join(screen.display) + "\n")
        if not fullscreen:
            expected = [step[1] for step in steps]

            def in_order():
                rows = [row.strip() for row in screen.display]
                assert [row for row in rows if row in expected] == expected, rows

            in_order()
            # Keep the width, which pyte does not reflow, and lose the rows:
            # only the session's repaint can bring them back.
            before_resize = len(raw)
            screen.resize(lines=12, columns=80)
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 12, 80, 0, 0))
            os.kill(process.pid, signal.SIGWINCH)
            receive(expected[-1], after=before_resize)
            in_order()
            (OUT / f"{name}-resized.txt").write_text("\n".join(screen.display) + "\n")
            return finish(fullscreen)
        before_resize = len(raw)
        screen.resize(lines=12, columns=40)
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 12, 40, 0, 0))
        os.kill(process.pid, signal.SIGWINCH)
        # Observe a complete frame painted after the resize before sending a key.
        receive("Wove", after=before_resize)
        # An additional key produces an observable update after the resize.
        os.write(master, b"+" if name == "counter" else b"x" if name == "editor" else b"\x01\x7f")
        receive("Count: 3" if name == "counter" else "bytes" if name == "editor" else "Text")
        check_border()
        (OUT / f"{name}-resized.txt").write_text("\n".join(screen.display) + "\n")
        finish(fullscreen)
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        (OUT / f"{name}.ansi").write_bytes(raw)
        os.close(master)
        os.close(slave)


def startup_signal():
    """A fatal signal while the startup probe waits must restore raw mode only."""
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
    original = termios.tcgetattr(slave)
    process = subprocess.Popen([str(ROOT / "target/debug/examples/gallery")],
                               stdin=slave, stdout=slave, stderr=slave,
                               start_new_session=True)
    raw = bytearray()
    try:
        deadline = time.monotonic() + 10
        while b"\x1b[6n" not in raw:
            assert time.monotonic() < deadline, "startup probe was not sent"
            if select.select([master], [], [], 0.05)[0]:
                raw.extend(os.read(master, 65536))
        assert termios.tcgetattr(slave) != original, "probe must run in raw mode"
        process.send_signal(signal.SIGTERM)
        assert process.wait(timeout=10) == -signal.SIGTERM
        while select.select([master], [], [], 0.05)[0]:
            raw.extend(os.read(master, 65536))
        assert termios.tcgetattr(slave) == original, "startup signal left raw mode enabled"
        assert b"\x1b[?1049l" not in raw, "startup must not leave a screen it never entered"
        assert b"\x1b[<1u" not in raw, "startup must not pop the shell's keyboard mode"
        print("gallery: startup signal restoration passed")
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        (OUT / "startup-signal.ansi").write_bytes(raw)
        os.close(master)
        os.close(slave)


scenarios = {
    "counter": [(b"++", "Count: 2")],
    "gallery": [(b"\x1b[200~scroll\x1b[201~", "A clipped viewport.")],
    "editor": [(b"\x1b[200~\nNew line\x1b[201~", "New line")],
    # The answers come from a worker thread and are drawn only if it wakes the loop.
    "inline": [(b"one\r", "echo: one"), (b"two\r", "echo: two")],
}
# Package CI selects its own examples; a local invocation without names runs all.
selected = sys.argv[1:] or list(scenarios)
if unknown := set(selected) - scenarios.keys():
    raise SystemExit(f"Unknown scenarios: {', '.join(sorted(unknown))}")
for name in selected:
    if name == "gallery":
        startup_signal()
    scenario(name, scenarios[name], fullscreen=name != "inline")
