import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from setup_scrfd import MODEL_SHA256, OUTPUT_NAMES, checked, detector_from_archive, save_exact
from verify_scrfd import verify_geometry


class ScrfdToolTests(unittest.TestCase):
    def test_pinned_archive_rejects_unknown_bytes_before_extraction(self):
        with self.assertRaisesRegex(ValueError, "pinned"):
            detector_from_archive(b"untrusted archive")

    def test_size_and_hash_are_both_required(self):
        import hashlib

        data = b"fixture"
        checksum = hashlib.sha256(data).hexdigest()
        self.assertEqual(checked(data, len(data), checksum, "fixture"), data)
        for size, pin in [(len(data) + 1, checksum), (len(data), "0" * 64)]:
            with self.assertRaises(ValueError):
                checked(data, size, pin, "fixture")

    def test_setup_reuses_identical_files_but_preserves_custom_local_content(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "model"
            save_exact(path, b"original")
            save_exact(path, b"original")
            with self.assertRaisesRegex(ValueError, "overwrite"):
                save_exact(path, b"replacement")
            self.assertEqual(path.read_bytes(), b"original")

    @staticmethod
    def capture():
        tensors = []
        for channels in (1, 4, 10):
            for stride in (8, 16, 32):
                count = 2 * (640 // stride) ** 2
                tensors.append({"shape": [count, channels], "values": [0.0] * (count * channels)})
        index = (12 * 80 + 10) * 2
        tensors[0]["values"][index] = 0.9
        tensors[3]["values"][index * 4 : index * 4 + 4] = [1.0, 2.0, 3.0, 4.0]
        tensors[6]["values"][index * 10 : index * 10 + 10] = [
            -0.5,
            -1.0,
            0.5,
            -1.0,
            0.0,
            0.0,
            -0.5,
            1.0,
            0.5,
            1.0,
        ]
        return {
            "model_sha256": MODEL_SHA256,
            "output_names": OUTPUT_NAMES,
            "width": 320,
            "height": 320,
            "scale": 2.0,
            "threshold": 0.55,
            "tensors": tensors,
            "detections": [
                {
                    "confidence": 0.9,
                    "bbox": [36.0, 40.0, 16.0, 24.0],
                    "keypoints": [
                        [38.0, 44.0],
                        [42.0, 44.0],
                        [40.0, 48.0],
                        [38.0, 52.0],
                        [42.0, 52.0],
                    ],
                }
            ],
        }

    def test_geometry_reconstructs_stride_scale_and_landmark_order(self):
        result = verify_geometry(self.capture())
        self.assertEqual(result["matched_detections"], 1)
        self.assertEqual(result["maximum_coordinate_or_score_error"], 0.0)

    def test_geometry_rejects_displaced_box_and_wrong_stride_head(self):
        report = self.capture()
        report["detections"][0]["bbox"][0] += 1
        with self.assertRaisesRegex(ValueError, "geometry"):
            verify_geometry(report)
        report = self.capture()
        report["tensors"][0]["shape"] = [3200, 1]
        with self.assertRaisesRegex(ValueError, "shape"):
            verify_geometry(report)

    def test_geometry_cannot_claim_evidence_with_empty_detections(self):
        report = self.capture()
        report["detections"] = []
        with self.assertRaisesRegex(ValueError, "at least one"):
            verify_geometry(report)


if __name__ == "__main__":
    unittest.main()
