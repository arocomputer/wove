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
import termios
import time
import pyte

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "artifacts/ui"
OUT.mkdir(parents=True, exist_ok=True)


def scenario(name, steps):
    """Run an example on a PTY and retain visible frames and terminal bytes."""
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
    original = termios.tcgetattr(slave)
    screen = pyte.Screen(80, 24)
    stream = pyte.ByteStream(screen)
    raw = bytearray()
    process = subprocess.Popen([str(ROOT / "target/debug/examples" / name)],
                               stdin=slave, stdout=slave, stderr=slave,
                               start_new_session=True, env={**os.environ, "TERM": "xterm-256color"})

    def receive(expected, after=-1):
        """Drain complete output until the expected frame appears or time expires."""
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            if select.select([master], [], [], 0.05)[0]:
                try:
                    chunk = os.read(master, 65536)
                except OSError as error:
                    if error.errno == errno.EIO:
                        break
                    raise
                raw.extend(chunk)
                stream.feed(chunk)
            if len(raw) > after and expected in "\n".join(screen.display):
                return
            if process.poll() is not None:
                break
        raise AssertionError(f"{name}: missing {expected!r}\n" + "\n".join(screen.display))

    def check_border():
        if name == "gallery":
            for row in screen.display[3:-2]:
                assert row[18] == "│" and row[-1] == "│", f"clipped panel border: {row!r}"

    try:
        receive("wove")
        for index, (keys, expected) in enumerate(steps):
            os.write(master, keys)
            receive(expected)
            check_border()
            (OUT / f"{name}-{index}.txt").write_text("\n".join(screen.display) + "\n")
        before_resize = len(raw)
        screen.resize(lines=12, columns=40)
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 12, 40, 0, 0))
        os.kill(process.pid, signal.SIGWINCH)
        # Observe the resize frame before sending a separate keyboard event.
        receive("wove", after=before_resize)
        # An additional key produces an observable update after the resize.
        os.write(master, b"+" if name == "counter" else b"\x01\x7f")
        receive("Count: 3" if name == "counter" else "Text")
        check_border()
        (OUT / f"{name}-resized.txt").write_text("\n".join(screen.display) + "\n")
        os.write(master, b"\x1b")
        assert process.wait(timeout=10) == 0
        while select.select([master], [], [], 0.05)[0]:
            chunk = os.read(master, 65536)
            raw.extend(chunk)
        restored = termios.tcgetattr(slave)
        assert restored == original, f"{name}: terminal attributes were not restored"
        assert b"\x1b[?1049l" in raw, "alternate screen was not restored"
        assert b"\x1b[?25h" in raw, "cursor was not restored"
        print(f"{name}: input, resize, and terminal restoration passed")
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        (OUT / f"{name}.ansi").write_bytes(raw)
        os.close(master)
        os.close(slave)


scenario("counter", [(b"++", "Count: 2")])
scenario("gallery", [(b"\x1b[200~scroll\x1b[201~", "A clipped viewport.")])
