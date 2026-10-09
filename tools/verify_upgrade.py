"""Verify schema-v2 upgrade and versioned analysis refresh on a real snapshot.

The supplied snapshot is opened read-only and backed up into a disposable
database. No decisions or source-writing operations are invoked.
"""
from contextlib import closing
import argparse
import hashlib
import json
from pathlib import Path
import sqlite3
import subprocess
import tempfile


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--database", type=Path, default=Path("artifacts/evaluation-v4-conservative.sqlite3"))
    parser.add_argument("--cli", type=Path, default=Path("target/release/iris-cli.exe"))
    parser.add_argument("--project", type=int, default=1)
    parser.add_argument("--models", type=Path, default=Path("models"))
    parser.add_argument("--expect-cache-reuse", action="store_true", help="Historical mode for a binary whose analysis version matches the snapshot")
    args = parser.parse_args()
    original = args.database.resolve(strict=True)
    binary = args.cli.resolve(strict=True)
    before = digest(original)
    with tempfile.TemporaryDirectory(prefix="iris-schema-upgrade-") as temporary:
        work = Path(temporary)
        database = work / "library.sqlite3"
        with closing(sqlite3.connect(original.as_uri() + "?mode=ro", uri=True)) as source:
            assert source.execute("pragma user_version").fetchone() == (2,)
            prior_count = source.execute("select count(*) from photos where project_id=?", (args.project,)).fetchone()[0]
            assert prior_count > 0
            with closing(sqlite3.connect(database)) as destination:
                source.backup(destination)

        def run(*arguments, with_models=False):
            model_dir = args.models.resolve() if with_models else work / "no-models"
            return subprocess.run([str(binary), "--database", str(database), "--model-dir", str(model_dir), *map(str, arguments)],
                                    cwd=work, text=True, capture_output=True, timeout=120)

        def cli(*arguments, with_models=False):
            result = run(*arguments, with_models=with_models)
            assert result.returncode == 0, result.stderr
            return json.loads(result.stdout)

        projects = cli("projects")
        assert any(p["id"] == args.project for p in projects)
        backup_paths = list(work.glob("library.v2.*.bak"))
        assert len(backup_paths) == 1, "upgrade must preserve exactly one pre-migration backup"
        with closing(sqlite3.connect(backup_paths[0])) as backup:
            assert backup.execute("pragma user_version").fetchone() == (2,)
            assert backup.execute("select count(*) from photos where project_id=?", (args.project,)).fetchone()[0] == prior_count
        with closing(sqlite3.connect(database)) as upgraded:
            assert upgraded.execute("pragma user_version").fetchone() == (3,)
            assert upgraded.execute("pragma quick_check").fetchone() == ("ok",)
        refreshed = 0
        manual_decisions_preserved = None
        if not args.expect_cache_reuse:
            stale = cli("photos", args.project)
            assert len(stale) == prior_count
            assert all(p["analysis_status"] == "stale" for p in stale)
            project = next(p for p in projects if p["id"] == args.project)
            assert project["pending_analysis"] == prior_count
            # This modifies only the disposable database copy, never source files.
            cli("decide", args.project, "keep", stale[0]["id"])
            with closing(sqlite3.connect(database)) as db:
                decisions_before = db.execute("select * from decisions order by photo_id").fetchall()
            failed = run("analyze", args.project)
            assert failed.returncode != 0, "old-version results were silently reused without models"
            assert all(p["analysis_status"] == "stale" for p in cli("photos", args.project))
            fresh = cli("analyze", args.project, with_models=True)
            assert fresh["analyzed"] == prior_count and fresh["reused"] == 0 and fresh["failed"] == 0, fresh
            refreshed = fresh["analyzed"]
            assert all(p["analysis_status"] == "current" for p in cli("photos", args.project))
            with closing(sqlite3.connect(database)) as db:
                assert db.execute("select * from decisions order by photo_id").fetchall() == decisions_before
            manual_decisions_preserved = True
        # Once current, the default provider can reuse every result without models.
        analysis = cli("analyze", args.project)
        assert analysis["reused"] == prior_count and analysis["analyzed"] == 0 and analysis["workers"] == 0, analysis
        assert cli("cache-history", args.project) == []
    assert digest(original) == before
    print(json.dumps({"schema_upgrade": "v2 to v3 passed", "backup_preserved": True,
                      "old_version_photos_reanalyzed": refreshed,
                      "manual_decisions_preserved": manual_decisions_preserved,
                      "current_cached_photos_reused_without_models": prior_count, "original_snapshot_unchanged": True}))


if __name__ == "__main__":
    main()
