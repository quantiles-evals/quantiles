import json
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from swebench_pro import _summarize_results, _write_first_gold_patch


class SweBenchProTest(unittest.TestCase):
    def test_write_first_gold_patch(self) -> None:
        with TemporaryDirectory() as directory:
            temp_dir = Path(directory)
            source = temp_dir / "gold.json"
            destination = temp_dir / "one.json"
            source.write_text(
                json.dumps(
                    [
                        {
                            "instance_id": "first",
                            "patch": "first patch",
                            "prefix": "gold",
                        },
                        {
                            "instance_id": "second",
                            "patch": "second patch",
                            "prefix": "gold",
                        },
                    ]
                )
            )

            _write_first_gold_patch(source, destination)

            self.assertEqual(
                json.loads(destination.read_text()),
                [{"instance_id": "first", "patch": "first patch", "prefix": "gold"}],
            )

    def test_summarize_results(self) -> None:
        with TemporaryDirectory() as directory:
            results = Path(directory) / "eval_results.json"
            results.write_text(json.dumps({"first": True, "second": False}))

            self.assertEqual(
                _summarize_results(results),
                {
                    "resolution_rate": 0.5,
                    "resolved_count": 1,
                    "total_count": 2,
                    "results_path": str(results),
                    "results": {"first": True, "second": False},
                },
            )

    def test_summarize_results_rejects_empty_results(self) -> None:
        with TemporaryDirectory() as directory:
            results = Path(directory) / "eval_results.json"
            results.write_text("{}")

            with self.assertRaisesRegex(ValueError, "returned no results"):
                _summarize_results(results)


if __name__ == "__main__":
    unittest.main()
