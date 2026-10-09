"""Known-answer checks for the sustained-load evidence verifier."""
import json
import sqlite3
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from benchmark_scale import verify_repeated_predictions


class PredictionConsistencyTests(unittest.TestCase):
    def setUp(self):
        self.database = sqlite3.connect(":memory:")
        self.addCleanup(self.database.close)
        self.database.executescript(
            "CREATE TABLE photos(id INTEGER PRIMARY KEY,path TEXT);"
            "CREATE TABLE analyses(photo_id INTEGER,data TEXT);"
        )
        self.paths = {"batch-000/a.jpg": 0, "batch-001/a.jpg": 0, "batch-000/b.jpg": 1}
        for index, (path, source) in enumerate(self.paths.items()):
            self.database.execute("INSERT INTO photos VALUES (?,?)", (index, path))
            self.database.execute("INSERT INTO analyses VALUES (?,?)", (index, json.dumps(
                {"elapsed_ms": index * 10, "score": source + 50, "faces": [{"confidence": 0.9}]}
            )))

    def test_timing_differences_and_distinct_sources_are_allowed(self):
        report = verify_repeated_predictions(self.database, self.paths)
        self.assertEqual(report["checked_analyses"], 3)
        self.assertEqual(report["unique_sources"], 2)

    def test_nested_prediction_corruption_is_rejected(self):
        self.database.execute("UPDATE analyses SET data=? WHERE photo_id=1", (
            json.dumps({"score": 50, "faces": [{"confidence": 0.8}]}),
        ))
        with self.assertRaisesRegex(AssertionError, "prediction differs"):
            verify_repeated_predictions(self.database, self.paths)

    def test_missing_analysis_is_rejected(self):
        self.database.execute("DELETE FROM analyses WHERE photo_id=1")
        with self.assertRaisesRegex(AssertionError, "no analysis"):
            verify_repeated_predictions(self.database, self.paths)


if __name__ == "__main__":
    unittest.main()
