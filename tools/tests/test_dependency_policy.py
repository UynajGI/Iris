"""Keep dependency updates reviewable without disabling security checks."""

import unittest
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]


class DependencyPolicyTests(unittest.TestCase):
    def test_all_dependency_roots_remain_covered(self):
        updates = yaml.safe_load((ROOT / ".github/dependabot.yml").read_text())["updates"]
        roots = {
            item["package-ecosystem"]: item.get("directories", [item.get("directory")])
            for item in updates
        }
        self.assertEqual(
            roots,
            {
                "github-actions": ["/"],
                "cargo": ["/", "/apps/shell/src-tauri", "/components/raw-decoder"],
                "npm": ["/", "/apps/shell"],
                "uv": ["/"],
            },
        )

    def test_version_noise_control_preserves_security_updates(self):
        updates = yaml.safe_load((ROOT / ".github/dependabot.yml").read_text())["updates"]
        for item in updates:
            with self.subTest(ecosystem=item["package-ecosystem"]):
                self.assertTrue(0 < item["open-pull-requests-limit"] <= 2)
                self.assertNotIn("ignore", item)
                groups = item["groups"]
                self.assertEqual(groups["maintenance-minor-patch"]["applies-to"], "version-updates")
                self.assertEqual(
                    groups["maintenance-minor-patch"]["update-types"], ["minor", "patch"]
                )
                self.assertEqual(
                    groups["security-fixes"],
                    {
                        "applies-to": "security-updates",
                        "patterns": ["*"],
                    },
                )
                if item["package-ecosystem"] == "cargo":
                    self.assertEqual(
                        set(groups["maintenance-minor-patch"]["exclude-patterns"]),
                        {"ort", "nalgebra", "base64", "tower-http"},
                    )

    def test_verify_runs_once_per_pr_and_still_checks_main(self):
        # BaseLoader preserves the YAML 1.2 workflow key "on" as text.
        workflow = yaml.load(
            (ROOT / ".github/workflows/verify.yml").read_text(), Loader=yaml.BaseLoader
        )
        triggers = workflow["on"]
        self.assertEqual(triggers["push"]["branches"], ["main"])
        for trigger in ["pull_request", "workflow_call", "workflow_dispatch"]:
            self.assertIn(trigger, triggers)


if __name__ == "__main__":
    unittest.main()
