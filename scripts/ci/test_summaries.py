"""Ensure the required gate cannot accept failed selection or unexpected skips."""
import copy
import json
import unittest
from unittest.mock import patch

from changes import plan
from complete import verify


class SummaryTests(unittest.TestCase):
    def setUp(self):
        printing = patch("builtins.print")
        printing.start()
        self.addCleanup(printing.stop)

    def needs(self, work):
        selected = plan(work)
        needs = {"changes": {"result": "success", "outputs": {"plan": json.dumps(selected)}}}
        flags = {"packages": bool(selected["packages"]["include"]),
                 "quality": selected["quality"]["run"], "guides": selected["guides"],
                 "website": selected["website"], "audit": any(selected["audits"].values())}
        needs.update({key: {"result": "success" if value else "skipped"} for key, value in flags.items()})
        return needs

    def test_unaffected_jobs_must_skip_and_affected_jobs_must_pass(self):
        for work in (set(), {"website"}, {"core", "quality", "guides", "audit"}):
            verify(self.needs(work))

    def test_failure_cancellation_and_unexpected_skips_fail_gate(self):
        for work in (set(), {"website", "core", "quality", "guides", "audit"}):
            baseline = self.needs(work)
            for job in baseline:
                for result in ("failure", "cancelled", "skipped", "success"):
                    if result == baseline[job]["result"]:
                        continue
                    needs = copy.deepcopy(baseline)
                    needs[job]["result"] = result
                    with self.subTest(job=job, result=result), self.assertRaises(ValueError):
                        verify(needs)

    def test_missing_or_malformed_plan_fails_closed(self):
        for raw in ("", "{}", '{"audits":{"rust":"false"}}'):
            needs = self.needs(set())
            needs["changes"]["outputs"]["plan"] = raw
            with self.assertRaises((ValueError, KeyError)):
                verify(needs)
