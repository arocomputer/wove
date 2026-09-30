"""Review or apply protections using an administrator's authenticated gh CLI."""
import argparse
import copy
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
REPO = "arocomputer/wove"
ACTION_APP = 15368


def api(path, method="GET", body=None):
    """Use gh's existing authentication without reading or printing credentials."""
    command = ["gh", "api", f"repos/{REPO}/{path}", "--method", method]
    if body is not None:
        command += ["--input", "-"]
    output = subprocess.check_output(command, input=json.dumps(body).encode() if body is not None else None)
    return json.loads(output) if output.strip() else None


def main_rule(current):
    """Preserve protections and review policy while replacing required checks."""
    body = {key: copy.deepcopy(current[key]) for key in
            ("name", "target", "enforcement", "conditions", "rules", "bypass_actors")}
    status = {"type": "required_status_checks", "parameters": {
        "strict_required_status_checks_policy": True, "do_not_enforce_on_create": False,
        "required_status_checks": [{"context": "CI", "integration_id": ACTION_APP}],
    }}
    body["rules"] = [rule for rule in body["rules"] if rule["type"] != "required_status_checks"] + [status]
    body["enforcement"] = "active"
    return body


def tag_rules():
    """Limit tag creation to admins; forbid moving or deleting release tags."""
    common = {"target": "tag", "enforcement": "active", "conditions": {
        "ref_name": {"include": ["refs/tags/v*"], "exclude": []},
    }}
    return [
        {**common, "name": "release-tag-creation", "bypass_actors": [
            {"actor_id": 5, "actor_type": "RepositoryRole", "bypass_mode": "always"},
        ], "rules": [{"type": "creation"}]},
        {**common, "name": "release-tags-immutable", "bypass_actors": [],
         "rules": [{"type": "update"}, {"type": "deletion"}]},
    ]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--apply", action="store_true", help="Apply after CI succeeds on main")
    args = parser.parse_args()
    rules = api("rulesets?includes_parents=true")
    matches = [rule for rule in rules if rule["name"] == "main" and rule["source"] == REPO and rule["target"] == "branch"]
    if len(matches) != 1:
        raise ValueError("expected one repository-owned main ruleset; inspect settings manually")
    current = api(f"rulesets/{matches[0]['id']}")
    changes = [main_rule(current), *tag_rules()]
    print(json.dumps(changes, indent=2))
    if not args.apply:
        print("review only; use --apply after this CI workflow is merged")
        return
    head = api("branches/main")["commit"]["sha"]
    checks = api(f"commits/{head}/check-runs?per_page=100")["check_runs"]
    gates = [check for check in checks if check["name"] == "CI" and check["app"]["id"] == ACTION_APP]
    if not gates or max(gates, key=lambda check: check["id"])["conclusion"] != "success":
        raise ValueError("CI must pass on current main before required-check migration")
    backup = ROOT / "artifacts/settings"
    backup.mkdir(parents=True, exist_ok=True)
    (backup / "main-before.json").write_text(json.dumps(current, indent=2) + "\n")
    for body in changes:
        existing = [rule for rule in rules if rule["name"] == body["name"] and rule["source"] == REPO]
        if existing:
            api(f"rulesets/{existing[0]['id']}", "PUT", body)
        else:
            api("rulesets", "POST", body)
    variables = api("actions/variables?per_page=100")["variables"]
    body = {"name": "WOVE_LEGACY_CHECKS", "value": "false"}
    if any(variable["name"] == body["name"] for variable in variables):
        api("actions/variables/WOVE_LEGACY_CHECKS", "PATCH", body)
    else:
        api("actions/variables", "POST", body)
    print("CI is required, release tags are protected, and legacy checks are disabled")


if __name__ == "__main__":
    main()
