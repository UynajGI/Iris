"""Exercise the merge policy without performing external writes."""

import importlib.util
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("automerge", ROOT / "tools/dependabot-automerge.py")
automerge = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(automerge)
REPO, HEAD, BASE = "owner/Iris", "a" * 40, "b" * 40


def pull_request():
    return {
        "number": 20,
        "state": "open",
        "draft": False,
        "user": {"login": automerge.BOT, "type": "Bot"},
        "head": {"sha": HEAD, "ref": "dependabot/npm/maintenance", "repo": {"full_name": REPO}},
        "base": {"sha": BASE, "ref": "main", "repo": {"full_name": REPO}},
        "mergeable_state": "clean",
    }


def commit(name="lefthook", version="2.2.0", update="minor"):
    return {
        "author": {"login": automerge.BOT, "id": 49699333},
        "committer": {"login": "web-flow", "id": 19864447},
        "commit": {
            "verification": {"verified": True, "reason": "valid"},
            "message": f"Bump dependency\n---\nupdated-dependencies:\n"
            f"- dependency-name: {name}\n  dependency-version: {version}\n"
            f"  update-type: version-update:semver-{update}\n...\nSigned-off-by: bot",
        },
    }


def workflow_runs():
    return [
        {
            "id": index,
            "path": path,
            "head_sha": HEAD,
            "status": "completed",
            "conclusion": "success",
            "event": "pull_request" if index == 1 else "dynamic",
            "pull_requests": [{"number": 20, "base": {"sha": BASE}}],
        }
        for index, path in enumerate(automerge.GATES, 1)
    ]


class DependabotAutomergeTests(unittest.TestCase):
    def test_api_scope_and_arguments_never_invoke_a_shell(self):
        with patch.object(automerge.subprocess, "run") as run:
            run.return_value.stdout = "{}"
            automerge.api(REPO, "pulls/20", "GET")
            self.assertFalse(run.call_args.kwargs["shell"])
            self.assertEqual(run.call_args.args[0][:4], ["gh", "api", "--method", "GET"])
            run.reset_mock()
            with self.assertRaises(ValueError):
                automerge.api("../escape; command", "pulls/20")
            run.assert_not_called()

    def test_only_current_same_repo_dependabot_main_pr_is_eligible(self):
        self.assertTrue(automerge.eligible_pr(pull_request(), REPO, HEAD))
        cases = [
            ("state", "closed"),
            ("draft", True),
            ("user.login", "human"),
            ("user.type", "User"),
            ("head.repo.full_name", "fork/Iris"),
            ("head.ref", "codex/feature"),
            ("head.sha", "new-head"),
            ("base.ref", "release"),
            ("base.repo.full_name", "other/Iris"),
        ]
        for key, value in cases:
            with self.subTest(key=key):
                pr = pull_request()
                target = pr
                parts = key.split(".")
                for part in parts[:-1]:
                    target = target[part]
                target[parts[-1]] = value
                self.assertFalse(automerge.eligible_pr(pr, REPO, HEAD))

    def test_stable_minor_patch_only_no_manual_dependencies(self):
        self.assertTrue(automerge.safe_dependency(automerge.dependency_metadata(commit())[0]))
        unsafe = [
            commit(update="major"),
            commit(version="0.9.1", update="patch"),
            commit(version="2.3.0-rc.1", update="patch"),
            commit(version="unknown"),
            *(commit(name=name) for name in automerge.MANUAL),
        ]
        for item in unsafe:
            with self.subTest(message=item["commit"]["message"]):
                self.assertFalse(automerge.safe_dependency(automerge.dependency_metadata(item)[0]))

    def test_verified_bot_commits_and_structured_metadata_required(self):
        for field, value in [("author", {"login": "human"}), ("author", {})]:
            item = commit()
            item[field] = value
            self.assertEqual(automerge.dependency_metadata(item), [])
        for verified, message in [(False, "unused"), (True, "no metadata"), (True, "\n---\n[")]:
            item = commit()
            item["commit"].update(verification={"verified": verified}, message=message)
            self.assertEqual(automerge.dependency_metadata(item), [])

    def test_only_modified_dependency_files_and_action_workflows(self):
        files = [{"filename": "package-lock.json", "status": "modified"}]
        self.assertTrue(automerge.safe_changes([commit()], files))
        for filename, status in [
            ("apps/shell/src/App.tsx", "modified"),
            ("tools/dependabot-automerge.py", "modified"),
            ("package.json", "added"),
            ("package-lock.json", "removed"),
            ("package.json", "renamed"),
        ]:
            self.assertFalse(
                automerge.safe_changes([commit()], [{"filename": filename, "status": status}])
            )
        self.assertFalse(automerge.safe_changes([], files))
        self.assertFalse(automerge.safe_changes([commit()], []))
        self.assertFalse(automerge.safe_changes([commit(), commit(name="glib")], files))

    def test_latest_run_and_every_job_must_succeed(self):
        runs = workflow_runs()
        retry = {**runs[0], "id": 10, "status": "in_progress", "conclusion": None}
        unrelated = {**retry, "id": 11, "head_sha": "old"}
        selected = automerge.latest_runs([*runs, retry, unrelated], HEAD)
        self.assertEqual(selected[runs[0]["path"]]["id"], 10)
        jobs = [{"name": "required", "status": "completed", "conclusion": "success"}]
        self.assertTrue(automerge.successful_jobs(jobs, {"required"}))
        self.assertFalse(automerge.successful_jobs(jobs, {"missing"}))
        for result in ["failure", "skipped", "cancelled", "neutral", None]:
            self.assertFalse(
                automerge.successful_jobs([{**jobs[0], "conclusion": result}], {"required"})
            )
        self.assertFalse(
            automerge.successful_jobs(
                [*jobs, {**jobs[0], "name": "extra", "conclusion": "failure"}], {"required"}
            )
        )

    def ci_pages(self, runs):
        def pages(repo, path, key=None):
            if path.startswith("actions/runs?"):
                return runs
            index = int(path.split("/")[2]) - 1
            return [
                {"name": name, "status": "completed", "conclusion": "success"}
                for name in automerge.GATES[runs[index]["path"]]
            ]

        return pages

    def test_both_workflows_same_head_and_tested_base_required(self):
        runs = workflow_runs()
        with patch.object(automerge, "pages", side_effect=self.ci_pages(runs)):
            self.assertEqual(automerge.ci_state(REPO, pull_request()), "ready")
            runs[0]["pull_requests"][0]["base"]["sha"] = "stale"
            self.assertEqual(automerge.ci_state(REPO, pull_request()), "blocked")
            runs[0]["status"] = "in_progress"
            self.assertEqual(automerge.ci_state(REPO, pull_request()), "pending")
            runs[0]["status"], runs[0]["conclusion"] = "completed", "failure"
            self.assertEqual(automerge.ci_state(REPO, pull_request()), "blocked")
        with patch.object(automerge, "pages", return_value=runs[:1]):
            self.assertEqual(automerge.ci_state(REPO, pull_request()), "pending")

    def final_api(self, current, merge=True):
        def api(repo, path, method="GET", **fields):
            if path.endswith("/merge"):
                self.assertEqual(method, "PUT")
                self.assertEqual(fields, {"sha": HEAD, "merge_method": "squash"})
                return {"merged": merge}
            if path == "git/ref/heads/main":
                return {"object": {"sha": BASE}}
            if path == "rules/branches/main":
                return [
                    {
                        "type": "required_status_checks",
                        "parameters": {
                            "strict_required_status_checks_policy": True,
                            "required_status_checks": [{"context": "rust"}],
                        },
                    }
                ]
            return current

        return api

    def final_pages(self, alerts=False):
        def pages(repo, path, key=None):
            if path.endswith("/commits"):
                return [commit()]
            if path.endswith("/files"):
                return [{"filename": "package-lock.json", "status": "modified"}]
            return [{"number": 1}] if alerts else []

        return pages

    def test_atomic_merge_only_after_all_final_guards(self):
        current = pull_request()
        with (
            patch.object(automerge, "pages", side_effect=self.final_pages()),
            patch.object(automerge, "api", side_effect=self.final_api(current)) as request,
        ):
            self.assertEqual(
                automerge.final_merge(REPO, pull_request()), "merged after complete CI"
            )
            self.assertEqual(request.call_args.kwargs, {"sha": HEAD, "merge_method": "squash"})
            for changed in ["sha", "base", "protection"]:
                current = pull_request()
                if changed == "sha":
                    current["head"]["sha"] = "changed"
                elif changed == "base":
                    current["base"]["sha"] = "changed"
                else:
                    current["mergeable_state"] = "blocked"
                request.side_effect = self.final_api(current)
                request.reset_mock()
                automerge.final_merge(REPO, pull_request())
                self.assertFalse(any(call.args[2:] == ("PUT",) for call in request.call_args_list))

    def test_alerts_and_api_failure_never_merge(self):
        with (
            patch.object(automerge, "pages", side_effect=self.final_pages(alerts=True)),
            patch.object(automerge, "api") as request,
        ):
            self.assertEqual(automerge.final_merge(REPO, pull_request()), "open CodeQL alerts")
            request.assert_not_called()
        with (
            patch.object(automerge, "pages", side_effect=self.final_pages()),
            patch.object(automerge, "api", side_effect=self.final_api(pull_request(), merge=False)),
        ):
            with self.assertRaises(ValueError):
                automerge.final_merge(REPO, pull_request())

    def test_pending_timeout_and_changed_head_do_not_merge(self):
        with (
            patch.object(automerge, "api", return_value=pull_request()),
            patch.object(automerge, "ci_state", return_value="pending"),
            patch.object(automerge, "final_merge") as merge,
        ):
            self.assertIn(
                "no merge",
                automerge.merge_if_ready(REPO, 20, HEAD, sleep=lambda _: None, attempts=2),
            )
            self.assertIn("changed", automerge.merge_if_ready(REPO, 20, "old"))
            merge.assert_not_called()

    def test_pagination_reads_all_pages(self):
        with patch.object(automerge, "api", side_effect=[[{}] * 100, [{}]]) as request:
            self.assertEqual(len(automerge.pages(REPO, "pulls/20/files")), 101)
            self.assertIn("page=2", request.call_args.args[1])

    def test_forged_bot_author_with_human_signature_is_rejected(self):
        for committer in [
            None,
            {},
            {"login": "human", "id": 123},
            {"login": "web-flow", "id": 123},
        ]:
            item = commit()
            item["committer"] = committer
            self.assertEqual(automerge.dependency_metadata(item), [])

    def test_actual_manifest_diff_must_only_change_named_dependency_versions(self):
        item = {
            "filename": "package.json",
            "status": "modified",
            "patch": '@@ -1 +1 @@\n-  "lefthook": "^2.1.0",\n+  "lefthook": "^2.2.0",',
        }
        self.assertTrue(automerge.safe_changes([commit()], [item]))
        for patch_text in [
            None,
            '-"scripts": {"build": "safe"}\n+"scripts": {"build": "evil"}',
            '-"other": "2.1.0"\n+"other": "2.2.0"',
            '-"lefthook": "^1.1.0"\n+"lefthook": "^2.2.0"',
            '-"scripts": "lefthook 2.1.0"\n+"scripts": "lefthook 2.2.0"',
            '-"lefthook": "^2.1.0"\n+"lefthook": "file:2.2.0"',
        ]:
            self.assertFalse(
                automerge.safe_changes([commit()], [{**item, "patch": patch_text or ""}])
            )
        self.assertFalse(
            automerge.safe_changes(
                [commit()], [{**item, "filename": ".github/workflows/verify.yml"}]
            )
        )

    def test_missing_or_non_strict_native_rules_fail_closed(self):
        for rules in [
            [],
            [
                {
                    "type": "required_status_checks",
                    "parameters": {
                        "strict_required_status_checks_policy": False,
                        "required_status_checks": [{}],
                    },
                }
            ],
        ]:
            with (
                patch.object(automerge, "pages", side_effect=self.final_pages()),
                patch.object(
                    automerge,
                    "api",
                    side_effect=lambda repo, path, *args, **kwargs: (
                        rules
                        if path == "rules/branches/main"
                        else self.final_api(pull_request())(repo, path, *args, **kwargs)
                    ),
                ) as request,
            ):
                self.assertIn("strict", automerge.final_merge(REPO, pull_request()))
                self.assertFalse(any(call.args[2:] == ("PUT",) for call in request.call_args_list))


if __name__ == "__main__":
    unittest.main()
