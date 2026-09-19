"""Exercise the actual Linux summary scripts with success, skip, and failure results."""
import os
from pathlib import Path
import subprocess
import textwrap
import unittest

ROOT = Path(__file__).resolve().parents[2]


@unittest.skipIf(os.name == "nt", "summary jobs execute on Linux")
class SummaryTests(unittest.TestCase):
    def status(self, area, selection, required, result):
        """Substitute GitHub's result values into the workflow's final shell step."""
        workflow = (ROOT / f".github/workflows/{area}.yml").read_text()
        script = textwrap.dedent(workflow.rsplit("run: |\n", 1)[1])
        script = script.replace("${{ needs.changes.result }}", selection)
        script = script.replace("${{ needs.changes.outputs.run }}", required)
        job = "check" if area == "quality" else "test"
        script = script.replace("${{ needs." + job + ".result }}", result)
        return subprocess.run(["sh", "-e", "-c", script], capture_output=True).returncode

    def test_only_success_or_explicitly_unaffected_work_passes(self):
        for area in ("core", "dioxus", "keymap", "ssh", "quality"):
            for required, result in (("true", "success"), ("false", "skipped")):
                with self.subTest(area=area, required=required, result=result):
                    self.assertEqual(self.status(area, "success", required, result), 0)

    def test_failed_detection_or_affected_work_cannot_pass(self):
        cases = [
            ("failure", "false", "skipped"),
            ("cancelled", "false", "skipped"),
            ("success", "", "skipped"),
            ("success", "true", "failure"),
            ("success", "true", "cancelled"),
            ("success", "true", "skipped"),
        ]
        for area in ("core", "dioxus", "keymap", "ssh", "quality"):
            for selection, required, result in cases:
                with self.subTest(area=area, selection=selection, required=required, result=result):
                    self.assertNotEqual(self.status(area, selection, required, result), 0)
