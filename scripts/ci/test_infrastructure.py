"""Exercise release provenance, settings migration, and generated-site failures."""
import os
import io
import json
import struct
import tarfile
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import release
import settings
from web import links
from ui_build import commands
from registry import payload


class RegistryTests(unittest.TestCase):
    def test_upload_preserves_verified_archive_and_dependency_metadata(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "fixture-1.2.3.crate"
            manifest = b'''[package]
name="fixture"
version="1.2.3"
description="Fixture"
license="MIT"
[dependencies.alias]
package="original"
version="=2.0.0"
optional=true
default-features=false
features=["small"]
[target.'cfg(windows)'.build-dependencies]
platform="1"
'''
            with tarfile.open(archive, "w:gz") as crate:
                member = tarfile.TarInfo("fixture-1.2.3/Cargo.toml")
                member.size = len(manifest)
                crate.addfile(member, io.BytesIO(manifest))
            body = payload(archive)
            length = struct.unpack("<I", body[:4])[0]
            metadata = json.loads(body[4:4 + length])
            offset = 4 + length
            self.assertEqual(struct.unpack("<I", body[offset:offset + 4])[0], archive.stat().st_size)
            self.assertEqual(body[offset + 4:], archive.read_bytes())
            alias = metadata["deps"][0]
            self.assertEqual((alias["name"], alias["explicit_name_in_toml"]), ("original", "alias"))
            self.assertTrue(alias["optional"])
            self.assertFalse(alias["default_features"])
            self.assertEqual(metadata["deps"][1]["target"], "cfg(windows)")
            self.assertEqual(metadata["deps"][1]["kind"], "build")


class TerminalBuildTests(unittest.TestCase):
    def test_counter_builds_only_its_adapter_example(self):
        self.assertEqual(commands(["counter"]), [["cargo", "build", "--locked", "-p", "wove-dioxus", "--example", "counter"]])

    def test_unknown_scenario_does_not_build_anything(self):
        with self.assertRaises(ValueError):
            commands(["typo"])


class ReleaseTests(unittest.TestCase):
    def test_release_requires_main_ancestry_and_exact_internal_versions(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            environment = {**os.environ, "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": os.devnull}

            def git(*args):
                return subprocess.check_output(["git", "-c", "user.name=Release test", "-c",
                                                "user.email=release@example.test", *args], cwd=root,
                                               env=environment, stderr=subprocess.DEVNULL).decode().strip()

            git("init", "-b", "main")
            manifest = root / "crates/adapter/Cargo.toml"
            manifest.parent.mkdir(parents=True)
            manifest.write_text('[package]\nname="adapter"\n[dependencies]\nwove={version="=1.2.3"}\n')
            git("add", ".")
            git("commit", "-m", "reviewed main")
            git("update-ref", "refs/remotes/origin/main", "HEAD")
            git("tag", "v1.2.3")
            with patch.object(release, "root", root), patch.object(release, "published", return_value=("1.2.3", ["wove", "adapter"])), \
                    patch.dict(os.environ, {"GITHUB_REF_TYPE": "tag", "GITHUB_REF_NAME": "v1.2.3"}):
                self.assertEqual(release.verify()[0], "1.2.3")
                git("switch", "-c", "unreviewed")
                (root / "new-code").write_text("unreviewed\n")
                git("add", ".")
                git("commit", "-m", "off main")
                git("tag", "-f", "v1.2.3")
                with self.assertRaises(subprocess.CalledProcessError):
                    release.verify()
                git("switch", "main")
                manifest.write_text(manifest.read_text().replace("=1.2.3", "=1.2.2"))
                git("add", ".")
                git("commit", "-m", "wrong dependency")
                git("update-ref", "refs/remotes/origin/main", "HEAD")
                git("tag", "-f", "v1.2.3")
                with self.assertRaisesRegex(ValueError, "must require"):
                    release.verify()
                with patch.dict(os.environ, {"GITHUB_REF_NAME": "v9.9.9"}), self.assertRaises(ValueError):
                    release.verify()


class SettingsTests(unittest.TestCase):
    def current(self):
        return {"id": 1, "name": "main", "source": settings.REPO, "target": "branch", "enforcement": "active",
                "bypass_actors": [], "conditions": {"ref_name": {"include": ["~DEFAULT_BRANCH"], "exclude": []}},
                "rules": [{"type": "pull_request", "parameters": {"required_approving_review_count": 0}},
                          {"type": "required_status_checks", "parameters": {"required_status_checks": []}}]}

    def test_check_migration_preserves_review_rules_and_no_bypass(self):
        original = self.current()
        body = settings.main_rule(original)
        self.assertEqual(body["rules"][0], original["rules"][0])
        self.assertEqual(body["bypass_actors"], [])
        self.assertEqual(body["rules"][1]["parameters"]["required_status_checks"], [{"context": "CI", "integration_id": 15368}])
        self.assertEqual(original, self.current())

    def test_tag_admin_creation_does_not_allow_tag_mutation(self):
        creation, immutable = settings.tag_rules()
        self.assertTrue(creation["bypass_actors"])
        self.assertEqual(immutable["bypass_actors"], [])
        self.assertEqual({rule["type"] for rule in immutable["rules"]}, {"update", "deletion"})

    def test_failed_gate_cannot_modify_protection_or_disable_compatibility(self):
        def response(path, method="GET", body=None):
            self.assertEqual(method, "GET")
            if path.startswith("rulesets?"):
                return [self.current()]
            if path == "rulesets/1":
                return self.current()
            if path == "branches/main":
                return {"commit": {"sha": "a" * 40}}
            return {"check_runs": [{"name": "CI", "app": {"id": 15368}, "id": 1, "conclusion": "failure"}]}
        with patch.object(settings, "api", side_effect=response), patch("sys.argv", ["settings.py", "--apply"]), \
                patch("builtins.print"), self.assertRaisesRegex(ValueError, "CI must pass"):
            settings.main()


class SiteTests(unittest.TestCase):
    def test_broken_links_and_fragments_fail_even_when_html_build_succeeds(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "docs").mkdir()
            (root / "index.html").write_text('<a href="/docs/#api">API</a>')
            page = root / "docs/index.html"
            page.write_text('<h1 id="api">API</h1><a href="../">Home</a>')
            links(root)
            page.write_text('<h1 id="changed">API</h1>')
            with self.assertRaisesRegex(ValueError, "missing anchor"):
                links(root)
            (root / "index.html").write_text('<a href="/missing/">Missing</a>')
            with self.assertRaisesRegex(ValueError, "missing file"):
                links(root)


ROOT = Path(__file__).resolve().parents[2]


class WorkflowTests(unittest.TestCase):
    def test_release_jobs_do_not_restore_build_caches(self):
        for name in ('publish.yml', 'registry.yml'):
            workflow = (ROOT / '.github/workflows' / name).read_text()
            self.assertNotIn('rust-cache@', workflow)
            self.assertNotIn('actions/cache@', workflow)

    def test_policy_gate_still_checks_workflow_security(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        selector = workflow.split('  changes:', 1)[1].split('  packages:', 1)[0]
        self.assertIn('run: ./x workflows', selector)

    def test_maintenance_and_fuzz_scopes_avoid_adapter_matrices(self):
        from changes import affected, plan
        for path in ('scripts/ci/report.py', '.github/workflows/cache-cleanup.yml', '.github/infra-tools.json'):
            selected = plan(affected([path]))
            self.assertEqual(selected['packages']['include'], [])
            self.assertFalse(selected['quality']['code'])
        selected = plan(affected(['fuzz/fuzz_targets/decoder.rs']))
        self.assertEqual(selected['packages']['include'], [])
        self.assertTrue(selected['quality']['fuzz'])
