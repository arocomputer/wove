"""Fail CI when selection failed or selected work did not finish successfully."""
import json
import os


def verify(needs):
    """Require success for selected jobs and an explicit skip for everything else."""
    if needs["changes"]["result"] != "success":
        raise ValueError("change selection failed")
    plan = json.loads(needs["changes"]["outputs"]["plan"])
    if set(plan["audits"]) != {"rust", "web", "python"} or any(
        type(value) is not bool for value in plan["audits"].values()
    ):
        raise ValueError("invalid audit selection")
    if set(plan['quality']) != {'run', 'code', 'ui', 'fuzz'} or any(
        type(value) is not bool for value in plan['quality'].values()
    ):
        raise ValueError('invalid quality selection')
    if (plan['quality']['code'] or plan['quality']['fuzz']) and not plan['quality']['run']:
        raise ValueError('selected quality work cannot skip its job')
    if not isinstance(plan["packages"]["include"], list):
        raise ValueError("invalid package matrix")
    selected = {
        "packages": bool(plan["packages"]["include"]),
        "quality": plan["quality"]["run"],
        "guides": plan["guides"],
        "website": plan["website"],
        "audit": any(plan["audits"].values()),
    }
    if any(type(value) is not bool for value in selected.values()):
        raise ValueError("invalid check selection")
    for job, required in selected.items():
        expected = "success" if required else "skipped"
        actual = needs[job]["result"]
        if actual != expected:
            raise ValueError(f"{job}: expected {expected}, got {actual}")
        print(f"{job}: {actual}")


if __name__ == "__main__":
    verify(json.loads(os.environ["CI_RESULTS"]))
