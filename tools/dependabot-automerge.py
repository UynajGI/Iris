"""Fail-closed Dependabot squash merging from trusted workflow_run code only."""

import json
import os
import re
import subprocess
import time
from pathlib import Path

import yaml

BOT = "dependabot[bot]"
MANUAL = {"ort", "ort-sys", "glib", "glib-sys", "nalgebra", "base64", "tower-http"}
MANIFESTS = {
    "Cargo.toml",
    "Cargo.lock",
    "package.json",
    "package-lock.json",
    "pyproject.toml",
    "uv.lock",
    "apps/shell/package.json",
    "apps/shell/package-lock.json",
    "apps/shell/src-tauri/Cargo.toml",
    "apps/shell/src-tauri/Cargo.lock",
    "components/raw-decoder/Cargo.toml",
    "components/raw-decoder/Cargo.lock",
}
GATES = {
    ".github/workflows/verify.yml": {
        "Make tooling (windows-latest)",
        "Make tooling (ubuntu-24.04)",
        "Make tooling (macos-14)",
        "Dependency and secret audit",
        "rust",
        "local-media",
        "application",
    },
    "dynamic/github-code-scanning/codeql": {
        "Analyze (actions)",
        "Analyze (javascript-typescript)",
        "Analyze (python)",
        "Analyze (rust)",
    },
}


def api(repo, path, method="GET", **fields):
    command = ["gh", "api", "--method", method, f"repos/{repo}/{path}"]
    for key, value in fields.items():
        command.extend(["-f", f"{key}={value}"])
    if not re.fullmatch(
        r"[A-Za-z0-9][A-Za-z0-9-]*/[A-Za-z0-9][A-Za-z0-9._-]*", repo
    ) or path.startswith(("/", "-")):
        raise ValueError("Invalid repository API scope")
    result = subprocess.run(command, check=True, capture_output=True, text=True, shell=False)
    return json.loads(result.stdout)


def pages(repo, path, key=None):
    result = []
    for page in range(1, 101):
        separator = "&" if "?" in path else "?"
        data = api(repo, f"{path}{separator}per_page=100&page={page}")
        batch = data[key] if key else data
        result.extend(batch)
        if len(batch) < 100:
            return result
    raise ValueError("Unexpected pagination size; refusing to merge")


def eligible_pr(pr, repo, head):
    return (
        pr["state"] == "open"
        and not pr["draft"]
        and pr["user"]["login"] == BOT
        and pr["user"]["type"] == "Bot"
        and pr["head"]["repo"]["full_name"] == repo
        and pr["head"]["ref"].startswith("dependabot/")
        and pr["head"]["sha"] == head
        and pr["base"]["ref"] == "main"
        and pr["base"]["repo"]["full_name"] == repo
    )


def dependency_metadata(commit):
    author, committer = commit.get("author") or {}, commit.get("committer") or {}
    verification = commit["commit"]["verification"]
    if author.get("login") != BOT or author.get("id") != 49699333:
        return []
    # Author fields can be forged. Only GitHub's verified web-flow signer is trusted.
    if committer.get("login") != "web-flow" or committer.get("id") != 19864447:
        return []
    if not verification["verified"] or verification.get("reason") != "valid":
        return []
    message = commit["commit"]["message"]
    if "\n---\n" not in message:
        return []
    document = message.split("\n---\n", 1)[1].split("\n...", 1)[0]
    try:
        data = yaml.safe_load(document)
    except yaml.YAMLError:
        return []
    return data.get("updated-dependencies", []) if isinstance(data, dict) else []


def safe_dependency(dependency):
    if not isinstance(dependency, dict):
        return False
    name = dependency.get("dependency-name")
    version = str(dependency.get("dependency-version", ""))
    return (
        isinstance(name, str)
        and name not in MANUAL
        and re.fullmatch(r"[1-9]\d*\.\d+\.\d+", version) is not None
        and dependency.get("update-type")
        in {"version-update:semver-patch", "version-update:semver-minor"}
    )


def safe_changes(commits, files):
    if not commits or not files:
        return False
    for commit in commits:
        dependencies = dependency_metadata(commit)
        if not isinstance(dependencies, list) or not dependencies:
            return False
        if not all(safe_dependency(dependency) for dependency in dependencies):
            return False
    dependencies = [d for c in commits for d in dependency_metadata(c)]
    return all(safe_file(item, dependencies) for item in files)


def safe_file(item, dependencies):
    if item["status"] != "modified":
        return False
    filename = item["filename"]
    if filename not in MANIFESTS:
        # Workflow edits, including action upgrades, stay manual: they are executable code.
        return False
    if filename.endswith(("Cargo.lock", "package-lock.json", "uv.lock")):
        # Lockfile resolution is trusted only with the verified GitHub bot provenance above.
        return True
    patch = item.get("patch", "")
    removed = [line[1:] for line in patch.splitlines() if line.startswith("-")]
    added = [line[1:] for line in patch.splitlines() if line.startswith("+")]
    return (
        bool(removed)
        and len(removed) == len(added)
        and all(
            safe_version_line(before, after, dependencies) for before, after in zip(removed, added)
        )
    )


def safe_version_line(before, after, dependencies):
    version = r"(?<![\w.])[~^<>=!]*\d+\.\d+\.\d+(?![\w.-])"
    if re.sub(version, "<version>", before) != re.sub(version, "<version>", after):
        return False
    old, new = re.findall(r"\d+\.\d+\.\d+", before), re.findall(r"\d+\.\d+\.\d+", after)
    if len(old) != 1 or len(new) != 1:
        return False
    prior, next_version = tuple(map(int, old[0].split("."))), tuple(map(int, new[0].split(".")))
    if prior[0] != next_version[0] or prior >= next_version:
        return False
    for dependency in dependencies:
        name = re.escape(dependency["dependency-name"])
        declaration = rf"^\s*[\"']?{name}[\"']?\s*[:=]|^\s*[\"']{name}[<>=~!]"
        if not re.search(declaration, after) or new[0] != dependency["dependency-version"]:
            continue
        return dependency["update-type"].endswith("minor") or prior[1] == next_version[1]
    return False


def strict_checks_enabled(rules):
    return any(
        rule.get("type") == "required_status_checks"
        and rule.get("parameters", {}).get("strict_required_status_checks_policy") is True
        and bool(rule["parameters"].get("required_status_checks"))
        for rule in rules
    )


def latest_runs(runs, head):
    latest = {}
    for run in sorted(runs, key=lambda item: item["id"], reverse=True):
        path = run["path"]
        if path in GATES and run["head_sha"] == head and path not in latest:
            latest[path] = run
    return latest


def successful_jobs(jobs, required):
    return required <= {job["name"] for job in jobs} and all(
        job["status"] == "completed" and job["conclusion"] == "success" for job in jobs
    )


def ci_state(repo, pr):
    runs = latest_runs(
        pages(repo, f"actions/runs?head_sha={pr['head']['sha']}", "workflow_runs"),
        pr["head"]["sha"],
    )
    if set(runs) != set(GATES):
        return "pending"
    for path, run in runs.items():
        if run["status"] != "completed":
            return "pending"
        if run["conclusion"] != "success":
            return "blocked"
        jobs = pages(repo, f"actions/runs/{run['id']}/jobs?filter=latest", "jobs")
        if not successful_jobs(jobs, GATES[path]):
            return "blocked"
    verify = runs[".github/workflows/verify.yml"]
    if verify["event"] != "pull_request":
        return "blocked"
    tested_base = {
        item["base"]["sha"] for item in verify["pull_requests"] if item["number"] == pr["number"]
    }
    return "ready" if tested_base == {pr["base"]["sha"]} else "blocked"


def merge_if_ready(repo, number, head, sleep=time.sleep, attempts=60):
    for _ in range(attempts):
        pr = api(repo, f"pulls/{number}")
        if not eligible_pr(pr, repo, head):
            return "ineligible or changed head"
        state = ci_state(repo, pr)
        if state == "blocked":
            return "CI blocked or stale base"
        if state == "ready":
            return final_merge(repo, pr)
        sleep(30)
    return "CI still pending; no merge performed"


def final_merge(repo, pr):
    number, head = pr["number"], pr["head"]["sha"]
    commits = pages(repo, f"pulls/{number}/commits")
    files = pages(repo, f"pulls/{number}/files")
    if not safe_changes(commits, files):
        return "manual dependency review required"
    alerts = pages(repo, f"code-scanning/alerts?state=open&ref=refs/pull/{number}/merge")
    if alerts:
        return "open CodeQL alerts"
    current = api(repo, f"pulls/{number}")
    base = api(repo, "git/ref/heads/main")["object"]["sha"]
    if not eligible_pr(current, repo, head) or current["base"]["sha"] != base:
        return "head or base changed"
    if current["base"]["sha"] != pr["base"]["sha"] or current["mergeable_state"] != "clean":
        return "branch protection, reviews or base update pending"
    if not strict_checks_enabled(api(repo, "rules/branches/main")):
        return "strict current-base checks required; no merge performed"
    # Atomic head check; native rulesets still enforce required checks and current base.
    result = api(repo, f"pulls/{number}/merge", "PUT", sha=head, merge_method="squash")
    if not result.get("merged"):
        raise ValueError("GitHub refused merge; branch protections were not bypassed")
    return "merged after complete CI"


def main():
    event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text("utf-8"))
    repo = os.environ["GITHUB_REPOSITORY"]
    run = event["workflow_run"]
    if run["event"] != "pull_request" or run["conclusion"] != "success":
        return
    if run["head_repository"]["full_name"] != repo:
        return
    for linked in run["pull_requests"]:
        number = int(linked["number"])
        result = merge_if_ready(repo, number, run["head_sha"])
        print(f"Dependabot PR #{number}: {result}")


if __name__ == "__main__":
    main()
