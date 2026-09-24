"""Select affected CI areas from changed paths without skipping required results."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[2]
PACKAGE_NAMES = {"core": "wove", "dioxus": "wove-dioxus", "keymap": "wove-keymap", "ssh": "wove-ssh", "gpu": "wove-gpu"}
PACKAGES = set(PACKAGE_NAMES)
CHECKS = PACKAGES | {"quality", "website", "audit"}
RUST = PACKAGES | {"core-ui", "dioxus-ui", "quality", "quality-rust"}
AREAS = RUST | {"website", "audit"}
WORKFLOWS = {
    "core": {"core", "core-ui", "quality-rust"},
    "dioxus": {"dioxus", "dioxus-ui", "quality-rust"},
    "keymap": {"keymap", "quality-rust"},
    "ssh": {"ssh", "quality-rust"},
    "gpu": {"gpu", "quality-rust"},
    "quality": {"quality", "quality-rust"},
    "website": {"website"},
    "security": {"audit"},
    "publish": {"quality", "quality-rust"},
}


def package_work(package):
    """Select a package, its consumers, and its terminal scenarios when present."""
    if package == "core":
        return RUST
    if package == "examples":
        return {"core-ui", "quality", "quality-rust"}
    result = {package, "quality", "quality-rust"}
    if package == "dioxus":
        result.add("dioxus-ui")
    return result


def affected(paths, lock_owners=None):
    """Follow the current crate dependencies; unknown paths conservatively run everything."""
    result = set()
    for path in paths:
        if path.startswith("crates/web/src/content/docs/"):
            result.add("website")
        elif path in {"AGENTS.md", "CONTRIBUTING.md", "SECURITY.md", "LICENSE"} or path.endswith("/AGENTS.md"):
            continue
        elif path == "README.md":
            # The homepage imports this file; other repository prose is not built.
            result.add("website")
        elif path.startswith("crates/") and path.endswith("/README.md") and len(Path(path).parts) == 3:
            continue
        elif path.startswith(("docs/", "contributing/")) and path.endswith((".md", ".mdx", ".txt")):
            continue
        elif path == "x" or path.startswith("scripts/ci/"):
            result.update(AREAS)
        elif path == "Cargo.lock":
            result.update({"quality", "quality-rust", "audit"})
            for package in PACKAGES if lock_owners is None else lock_owners:
                result.update(package_work(package))
        elif path in {"Cargo.toml", "rust-toolchain.toml"}:
            result.update(RUST | {"audit"})
        elif path.startswith("crates/core/examples/"):
            result.update({"core-ui", "quality", "quality-rust"})
        elif path.startswith("crates/dioxus/examples/"):
            result.update({"dioxus-ui", "quality", "quality-rust"})
        elif path.startswith("crates/core/"):
            result.update(RUST)
        elif path.startswith("crates/web/"):
            result.add("website")
        elif path.startswith("crates/examples/"):
            result.update({"core-ui", "quality", "quality-rust"})
        elif any(path.startswith(f"crates/{package}/") for package in PACKAGES - {"core"}):
            result.update(package_work(path.split("/")[1]))
        elif path == "scripts/ui.py" or path.startswith("scripts/ui/"):
            result.update({"core-ui", "dioxus-ui", "quality"})
            if path.startswith("scripts/ui/frames/"):
                # The website shows these goldens as example screens.
                result.add("website")
        elif path in {"scripts/package.py", "scripts/release.py"}:
            result.update({"quality", "quality-rust"})
        elif path.startswith("scripts/"):
            result.add("quality")
        elif path.startswith(".github/workflows/") and path.endswith((".yml", ".yaml")):
            result.update(WORKFLOWS.get(Path(path).stem, AREAS) | {"quality"})
        elif path.startswith(".github/") and (
            path.endswith(".md") or path.startswith(".github/ISSUE_TEMPLATE/")
            or path in {".github/CODEOWNERS", ".github/dependabot.yml"}
        ):
            continue
        else:
            result.update(AREAS)
        if path.endswith("Cargo.toml"):
            result.add("audit")
    return result


def lockfile_owners(before, after):
    """Find crates reaching changed lockfile records through old or new dependency graphs."""
    if before.get("version") != after.get("version"):
        return None
    graphs = []
    for document in (before, after):
        records = document.get("package", [])
        graph = {(row["name"], row["version"], row.get("source", "")): row for row in records}
        if not graph or len(graph) != len(records):
            return None
        graphs.append(graph)
    old, new = graphs
    changed = {key for key in old.keys() | new.keys() if old.get(key) != new.get(key)}
    owners = set()
    for area, name in {**PACKAGE_NAMES, "examples": "wove-examples"}.items():
        for graph in graphs:
            pending = [key for key in graph if key[0] == name and not key[2]]
            if not pending:
                return None
            visited = set()
            while pending:
                key = pending.pop()
                if key in visited:
                    continue
                visited.add(key)
                if key in changed:
                    owners.add(area)
                for reference in graph[key].get("dependencies", []):
                    parts = reference.split(" ", 2)
                    targets = [candidate for candidate in graph
                               if candidate[0] == parts[0]
                               and (len(parts) < 2 or candidate[1] == parts[1])
                               and (len(parts) < 3 or candidate[2] == parts[2].strip("()"))]
                    if len(targets) != 1:
                        return None
                    pending.extend(targets)
    return owners


def changed_lock_owners(event_name, event, root=ROOT):
    """Read lockfiles from the same revisions as the diff; missing history checks all crates."""
    if event_name == "pull_request":
        before = event["pull_request"]["base"]["sha"]
        after = event["pull_request"]["head"]["sha"]
        before = subprocess.check_output(["git", "merge-base", before, after], cwd=root).decode().strip()
    else:
        before, after = event["before"], event["after"]
    documents = []
    for revision in (before, after):
        result = subprocess.run(["git", "show", f"{revision}:Cargo.lock"], cwd=root,
                                capture_output=True)
        if result.returncode:
            return None
        documents.append(tomllib.loads(result.stdout.decode()))
    return lockfile_owners(*documents)


def selection(area, work):
    """Expose separate Rust and terminal decisions without adding more platform jobs."""
    code = ("quality-rust" if area == "quality" else area) in work
    ui = f"{area}-ui" in work
    return {"run": area in work or ui, "code": code, "ui": ui}


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
    subprocess.run(["git", "diff", "--check", *revisions, "--"], cwd=root, check=True)
    output = subprocess.check_output(
        ["git", "diff", "--no-renames", "--name-only", "-z", *revisions, "--"], cwd=root,
    )
    return [os.fsdecode(path) for path in output.split(b"\0") if path]


def main():
    """Emit a boolean Actions output; detection failures exit nonzero rather than skipping tests."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("area", choices=sorted(CHECKS))
    args = parser.parse_args()
    event_path = os.environ.get("GITHUB_EVENT_PATH")
    event = json.loads(Path(event_path).read_text()) if event_path else {}
    event_name = os.environ.get("GITHUB_EVENT_NAME", "workflow_dispatch")
    paths = changed_paths(event_name, event)
    owners = changed_lock_owners(event_name, event) if paths and "Cargo.lock" in paths else None
    work = AREAS if paths is None else affected(paths, owners)
    selected = selection(args.area, work)
    if output := os.environ.get("GITHUB_OUTPUT"):
        with open(output, "a", encoding="utf-8") as stream:
            for key, value in selected.items():
                stream.write(f"{key}={str(value).lower()}\n")
    print(f"{args.area}: {selected}")


if __name__ == "__main__":
    main()
