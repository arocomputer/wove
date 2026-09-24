"""Pin dependency propagation and fail-closed Git diff handling for selective CI."""
from pathlib import Path
import copy
import os
import subprocess
import tempfile
import tomllib
import unittest
from unittest.mock import patch

from changes import AREAS, ROOT, RUST, affected, changed_paths, lockfile_owners, main, selection


class ChangeTests(unittest.TestCase):
    def test_core_retests_every_consumer(self):
        self.assertEqual(affected(["crates/core/src/tree.rs"]), RUST)

    def test_adapter_changes_do_not_retest_siblings(self):
        for package in ("dioxus", "keymap", "ssh", "gpu"):
            with self.subTest(package=package):
                expected = {package, "quality", "quality-rust"}
                if package == "dioxus":
                    expected.add("dioxus-ui")
                self.assertEqual(affected([f"crates/{package}/src/lib.rs"]), expected)

    def test_local_dependency_edges_are_covered(self):
        for manifest in (ROOT / "crates").glob("*/Cargo.toml"):
            # The application examples have no tests; the shared lint compiles them.
            consumer = "quality-rust" if manifest.parent.name == "examples" else manifest.parent.name
            data = tomllib.loads(manifest.read_text())
            for section in ("dependencies", "dev-dependencies", "build-dependencies"):
                for dependency in data.get(section, {}).values():
                    if isinstance(dependency, dict) and "path" in dependency:
                        source = (manifest.parent / dependency["path"]).resolve().relative_to(ROOT)
                        with self.subTest(consumer=consumer, dependency=str(source)):
                            self.assertIn(consumer, affected([f"{source.as_posix()}/src/lib.rs"]))

    def test_website_and_homepage_source_select_website(self):
        self.assertEqual(affected(["crates/web/src/pages/index.astro"]), {"website"})
        self.assertEqual(affected(["README.md"]), {"website"})
        self.assertEqual(affected(["crates/web/src/content/docs/start.mdx"]), {"website"})
        self.assertEqual(affected(["crates/web/src/content/docs/README.md"]), {"website"})
        self.assertEqual(affected(["crates/web/src/content/docs/AGENTS.md"]), {"website"})

    def test_manifests_and_lockfile_retest_rust_and_audit(self):
        for path in ("Cargo.lock", "Cargo.toml", "rust-toolchain.toml"):
            with self.subTest(path=path):
                self.assertEqual(affected([path]), RUST | {"audit"})
        self.assertEqual(affected(["crates/ssh/Cargo.toml"]), {"ssh", "quality", "quality-rust", "audit"})

    def test_terminal_harness_covers_core_and_dioxus_examples(self):
        self.assertEqual(affected(["scripts/ui.py"]), {"core-ui", "dioxus-ui", "quality"})
        self.assertEqual(affected(["crates/examples/src/editor.rs"]), {"core-ui", "quality", "quality-rust"})

    def test_frame_goldens_also_rebuild_the_website_that_shows_them(self):
        self.assertEqual(affected(["scripts/ui/frames/gallery-0.txt"]), {"core-ui", "dioxus-ui", "quality", "website"})

    def test_workflows_and_shared_ci_do_not_skip_their_checks(self):
        self.assertEqual(affected([".github/workflows/ssh.yml"]), {"ssh", "quality", "quality-rust"})
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
            (root / "crates/ssh/example.txt").write_bytes(b"example\n")
            git("add", ".")
            git("commit", "-m", "base")
            git("branch", "topic")
            (root / "unrelated-base-file").write_bytes(b"base change\n")
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
            self.assertEqual(affected(paths), {"ssh", "dioxus", "dioxus-ui", "quality", "quality-rust"})

    def test_manual_and_scheduled_runs_do_not_filter(self):
        for event in ("workflow_dispatch", "schedule"):
            self.assertIsNone(changed_paths(event, {}))

    def test_failed_diff_does_not_turn_into_a_skip(self):
        event = {"before": "a" * 40, "after": "b" * 40}
        with patch("changes.subprocess.run"), \
                patch("changes.subprocess.check_output", side_effect=subprocess.CalledProcessError(1, "git")):
            with self.assertRaises(subprocess.CalledProcessError):
                changed_paths("push", event)
        with self.assertRaises(ValueError):
            changed_paths("push", {"before": "--bad-revision", "after": "b" * 40})

    def test_readme_changes_do_not_run_rust_or_terminal_tests(self):
        work = affected(["crates/core/README.md"])
        self.assertEqual(selection("core", work), {"run": False, "code": False, "ui": False})
        self.assertEqual(selection("quality", work), {"run": False, "code": False, "ui": False})

    def test_non_published_docs_do_not_select_builds_or_tests(self):
        for path in ("AGENTS.md", "CONTRIBUTING.md", "SECURITY.md", "LICENSE",
                     "docs/design.md", "contributing/releases.md",
                     "crates/core/AGENTS.md", "crates/core/README.md", "crates/web/README.md",
                     ".github/pull_request_template.md", ".github/ISSUE_TEMPLATE/bug.yml",
                     ".github/CODEOWNERS", ".github/dependabot.yml", ".github/workflows/README.md"):
            with self.subTest(path=path):
                self.assertEqual(affected([path]), set())

    def test_documentation_does_not_hide_an_accompanying_code_change(self):
        self.assertEqual(affected(["AGENTS.md", "crates/core/src/lib.rs"]), RUST)
        self.assertEqual(affected([".github/actions/build/action.yml"]), AREAS)

    def test_invalid_diff_cannot_skip_all_jobs(self):
        with patch("changes.subprocess.run", side_effect=subprocess.CalledProcessError(2, "git")):
            with self.assertRaises(subprocess.CalledProcessError):
                changed_paths("push", {"before": "a" * 40, "after": "b" * 40})

    def test_ui_only_changes_emit_terminal_selection_without_unit_tests(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "event.json").write_text("{}")
            env = {"GITHUB_EVENT_PATH": str(root / "event.json"),
                   "GITHUB_EVENT_NAME": "pull_request", "GITHUB_OUTPUT": str(root / "output")}
            with patch.dict(os.environ, env, clear=True), patch("sys.argv", ["changes.py", "core"]), \
                    patch("changes.changed_paths", return_value=["scripts/ui/requirements.txt"]):
                main()
            self.assertEqual(dict(line.split("=", 1) for line in (root / "output").read_text().splitlines()),
                             {"run": "true", "code": "false", "ui": "true"})


class LockfileTests(unittest.TestCase):
    def setUp(self):
        """Model Core consumers plus independent SSH and example-only registry dependencies."""
        self.before = {"version": 4, "package": [
            {"name": "wove", "version": "0.0.1", "dependencies": ["render"]},
            {"name": "wove-dioxus", "version": "0.0.1", "dependencies": ["wove"]},
            {"name": "wove-keymap", "version": "0.0.1", "dependencies": ["wove"]},
            {"name": "wove-ssh", "version": "0.0.1", "dependencies": ["wove", "tokio"]},
            {"name": "wove-gpu", "version": "0.0.1", "dependencies": ["wove", "gfx"]},
            {"name": "wove-examples", "version": "0.0.1", "dependencies": ["wove", "demo"]},
            {"name": "tokio", "version": "1.0.0", "source": "registry", "dependencies": ["bytes"]},
            {"name": "bytes", "version": "1.0.0", "source": "registry"},
            {"name": "render", "version": "1.0.0", "source": "registry"},
            {"name": "demo", "version": "1.0.0", "source": "registry"},
            {"name": "gfx", "version": "1.0.0", "source": "registry"},
        ]}
        self.after = copy.deepcopy(self.before)

    def bump(self, name):
        """Update one registry dependency while leaving workspace source unchanged."""
        next(row for row in self.after["package"] if row["name"] == name)["version"] = "2.0.0"

    def test_ssh_transitive_dependency_does_not_select_core(self):
        self.bump("bytes")
        owners = lockfile_owners(self.before, self.after)
        self.assertEqual(owners, {"ssh"})
        self.assertEqual(affected(["Cargo.lock"], owners), {"ssh", "quality", "quality-rust", "audit"})

    def test_core_dependency_selects_all_consumers(self):
        self.bump("render")
        self.assertEqual(affected(["Cargo.lock"], lockfile_owners(self.before, self.after)), RUST | {"audit"})

    def test_example_dependency_selects_terminal_without_core_unit_tests(self):
        self.bump("demo")
        work = affected(["Cargo.lock"], lockfile_owners(self.before, self.after))
        self.assertEqual(selection("core", work), {"run": True, "code": False, "ui": True})

    def test_removed_dependency_still_checks_its_former_consumer(self):
        self.after["package"] = [row for row in self.after["package"] if row["name"] != "demo"]
        next(row for row in self.after["package"] if row["name"] == "wove-examples")["dependencies"] = ["wove"]
        self.assertEqual(lockfile_owners(self.before, self.after), {"examples"})

    def test_unknown_lockfile_relationships_fall_back_to_all_rust_checks(self):
        self.after["package"][0]["dependencies"] = ["missing-package"]
        self.assertIsNone(lockfile_owners(self.before, self.after))
        self.assertEqual(affected(["Cargo.lock"], None), RUST | {"audit"})

    def test_ambiguous_dependency_versions_do_not_allow_a_skip(self):
        self.after["package"].append({"name": "bytes", "version": "2.0.0", "source": "registry"})
        self.assertIsNone(lockfile_owners(self.before, self.after))
