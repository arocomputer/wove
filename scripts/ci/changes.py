"""Select affected CI areas from changed paths without skipping required results."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[2]
PACKAGES = {"core", "dioxus", "keymap", "ssh"}
RUST = PACKAGES | {"quality"}
AREAS = RUST | {"website", "audit"}
WORKFLOWS = {
    "core": {"core"},
    "dioxus": {"dioxus"},
    "keymap": {"keymap"},
    "ssh": {"ssh"},
    "quality": {"quality"},
    "website": {"website"},
    "deploy": {"website"},
    "security": {"audit"},
    "publish": {"quality"},
}


def affected(paths):
    """Follow the current crate dependencies; unknown paths conservatively run everything."""
    result = set()
    for path in paths:
        if path == "x" or path.startswith("scripts/ci/"):
            result.update(AREAS)
        elif path in {"Cargo.toml", "Cargo.lock", "rust-toolchain.toml"} or path.endswith("/Cargo.toml"):
            # Manifest edits can introduce dependencies not represented by today's graph.
            result.update(RUST | {"audit"})
        elif path.startswith("crates/core/"):
            result.update(RUST)
        elif path.startswith("crates/web/"):
            result.add("website")
        elif path.startswith("crates/examples/"):
            result.update({"core", "quality"})
        elif any(path.startswith(f"crates/{package}/") for package in PACKAGES - {"core"}):
            result.update({path.split("/")[1], "quality"})
        elif path == "scripts/ui.py" or path.startswith("scripts/ui/"):
            result.update({"core", "dioxus", "quality"})
        elif path.startswith("scripts/"):
            result.add("quality")
        elif path.startswith(".github/workflows/"):
            result.update(WORKFLOWS.get(Path(path).stem, AREAS) | {"quality"})
        elif path.startswith(".github/"):
            result.add("quality")
        elif path in {"README.md", "CONTRIBUTING.md", "AGENTS.md", "SECURITY.md"}:
            result.update({"quality", "website"})
        elif path == "LICENSE":
            result.add("quality")
        else:
            result.update(AREAS)
    return result


def changed_paths(event_name, event, root=ROOT):
    """Use the PR merge base or push range; manual/scheduled runs always check everything."""
    if event_name == "pull_request":
        before = event["pull_request"]["base"]["sha"]
        after = event["pull_request"]["head"]["sha"]
    elif event_name == "push":
        before, after = event["before"], event["after"]
    else:
        return None
    for revision in (before, after):
        if not re.fullmatch(r"(?:[0-9a-f]{40}|[0-9a-f]{64})", revision):
            raise ValueError("Invalid commit SHA in CI event")
    if set(before) == {"0"}:
        return None
    revisions = [f"{before}...{after}"] if event_name == "pull_request" else [before, after]
    output = subprocess.check_output(
        ["git", "diff", "--no-renames", "--name-only", "-z", *revisions, "--"], cwd=root,
    )
    return [os.fsdecode(path) for path in output.split(b"\0") if path]


def main():
    """Emit a boolean Actions output; detection failures exit nonzero rather than skipping tests."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("area", choices=sorted(AREAS))
    args = parser.parse_args()
    event_path = os.environ.get("GITHUB_EVENT_PATH")
    event = json.loads(Path(event_path).read_text()) if event_path else {}
    paths = changed_paths(os.environ.get("GITHUB_EVENT_NAME", "workflow_dispatch"), event)
    required = paths is None or args.area in affected(paths)
    value = "true" if required else "false"
    if output := os.environ.get("GITHUB_OUTPUT"):
        with open(output, "a", encoding="utf-8") as stream:
            stream.write(f"run={value}\n")
    print(f"{args.area}: {'required' if required else 'not affected'}")


if __name__ == "__main__":
    main()
