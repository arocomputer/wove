"""Pin dependency propagation and fail-closed Git diff handling for selective CI."""
from pathlib import Path
import os
import subprocess
import tempfile
import tomllib
import unittest
from unittest.mock import patch

from changes import AREAS, ROOT, RUST, affected, changed_paths


class ChangeTests(unittest.TestCase):
    def test_core_retests_every_consumer(self):
        self.assertEqual(affected(["crates/core/src/tree.rs"]), RUST)

    def test_adapter_changes_do_not_retest_siblings(self):
        for package in ("dioxus", "keymap", "ssh"):
            with self.subTest(package=package):
                self.assertEqual(affected([f"crates/{package}/src/lib.rs"]), {package, "quality"})

    def test_local_dependency_edges_are_covered(self):
        for manifest in (ROOT / "crates").glob("*/Cargo.toml"):
            consumer = "core" if manifest.parent.name == "examples" else manifest.parent.name
            data = tomllib.loads(manifest.read_text())
            for section in ("dependencies", "dev-dependencies", "build-dependencies"):
                for dependency in data.get(section, {}).values():
                    if isinstance(dependency, dict) and "path" in dependency:
                        source = (manifest.parent / dependency["path"]).resolve().relative_to(ROOT)
                        with self.subTest(consumer=consumer, dependency=str(source)):
                            self.assertIn(consumer, affected([f"{source.as_posix()}/src/lib.rs"]))

    def test_website_and_homepage_source_select_website(self):
        self.assertEqual(affected(["crates/web/src/pages/index.astro"]), {"website"})
        self.assertIn("website", affected(["README.md"]))

    def test_manifests_and_lockfile_retest_rust_and_audit(self):
        for path in ("Cargo.lock", "Cargo.toml", "crates/ssh/Cargo.toml", "rust-toolchain.toml"):
            with self.subTest(path=path):
                self.assertEqual(affected([path]), RUST | {"audit"})

    def test_terminal_harness_covers_core_and_dioxus_examples(self):
        self.assertEqual(affected(["scripts/ui.py"]), {"core", "dioxus", "quality"})
        self.assertEqual(affected(["crates/examples/src/editor.rs"]), {"core", "quality"})

    def test_workflows_and_shared_ci_do_not_skip_their_checks(self):
        self.assertEqual(affected([".github/workflows/ssh.yml"]), {"ssh", "quality"})
        self.assertEqual(affected(["scripts/ci/changes.py"]), AREAS)
        self.assertEqual(affected(["x"]), AREAS)
        self.assertEqual(affected(["new-build-input"]), AREAS)

    def test_pr_diff_ignores_unrelated_base_changes_and_includes_both_rename_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)

            def git(*args):
                """Run isolated history operations with a test-only identity."""
                return subprocess.check_output([
                    "git", "-c", "user.name=CI test", "-c", "user.email=ci@example.test", *args,
                ], cwd=root, stderr=subprocess.DEVNULL, env={
                    **os.environ, "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": os.devnull,
                }).decode().strip()

            git("init", "-b", "main")
            (root / "crates/ssh").mkdir(parents=True)
            (root / "crates/ssh/example.txt").write_text("example\n")
            git("add", ".")
            git("commit", "-m", "base")
            git("branch", "topic")
            (root / "unrelated-base-file").write_text("base change\n")
            git("add", ".")
            git("commit", "-m", "main advanced")
            base = git("rev-parse", "HEAD")
            git("switch", "topic")
            (root / "crates/dioxus").mkdir()
            (root / "crates/ssh/example.txt").rename(root / "crates/dioxus/example.txt")
            git("add", "-A")
            git("commit", "-m", "move example")
            head = git("rev-parse", "HEAD")
            paths = changed_paths("pull_request", {"pull_request": {
                "base": {"sha": base}, "head": {"sha": head},
            }}, root)
            self.assertEqual(set(paths), {"crates/ssh/example.txt", "crates/dioxus/example.txt"})
            self.assertEqual(affected(paths), {"ssh", "dioxus", "quality"})

    def test_manual_and_scheduled_runs_do_not_filter(self):
        for event in ("workflow_dispatch", "schedule"):
            self.assertIsNone(changed_paths(event, {}))

    def test_failed_diff_does_not_turn_into_a_skip(self):
        event = {"before": "a" * 40, "after": "b" * 40}
        with patch("changes.subprocess.check_output", side_effect=subprocess.CalledProcessError(1, "git")):
            with self.assertRaises(subprocess.CalledProcessError):
                changed_paths("push", event)
        with self.assertRaises(ValueError):
            changed_paths("push", {"before": "--bad-revision", "after": "b" * 40})
