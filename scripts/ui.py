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

    def receive(expected):
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
            if expected in "\n".join(screen.display):
                return
            if process.poll() is not None:
                break
        raise AssertionError(f"{name}: missing {expected!r}\n" + "\n".join(screen.display))

    try:
        receive("weft")
        for index, (keys, expected) in enumerate(steps):
            os.write(master, keys)
            receive(expected)
            (OUT / f"{name}-{index}.txt").write_text("\n".join(screen.display) + "\n")
        screen.resize(lines=12, columns=40)
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 12, 40, 0, 0))
        os.kill(process.pid, signal.SIGWINCH)
        # An additional key produces an observable update after the resize.
        os.write(master, b"+" if name == "counter" else b"k")
        receive("3" if name == "counter" else "Browse a shared collect")
        (OUT / f"{name}-resized.txt").write_text("\n".join(screen.display) + "\n")
        os.write(master, b"q")
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


scenario("counter", [(b"++", "2")])
scenario("explorer", [(b"j", "Browse a shared collection."), (b"j", "Make something useful.")])
