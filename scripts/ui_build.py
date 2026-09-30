"""Build only the binaries used by selected terminal scenarios."""
import subprocess
import sys

SCENARIOS = ("counter", "gallery", "editor", "inline", "grid", "files", "logs")


def commands(names):
    """Reject unknown scenarios before building unrelated workspace examples."""
    names = list(names) if names else list(SCENARIOS)
    unknown = set(names) - set(SCENARIOS)
    if unknown:
        raise ValueError(f"unknown PTY scenarios: {sorted(unknown)}")
    result = []
    core = [name for name in names if name not in {"counter", "editor"}]
    if core:
        result.append(["cargo", "build", "--locked", "-p", "wove", *[flag for name in core for flag in ("--example", name)]])
    if "counter" in names:
        result.append(["cargo", "build", "--locked", "-p", "wove-dioxus", "--example", "counter"])
    if "editor" in names:
        result.append(["cargo", "build", "--locked", "-p", "wove-examples", "--bin", "editor"])
    return result


if __name__ == "__main__":
    for command in commands(sys.argv[1:]):
        subprocess.run(command, check=True)
