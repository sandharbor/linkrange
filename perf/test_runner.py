import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from run import SCENARIOS, compare, prepare_retry, samples, write_json


class ComparisonTests(unittest.TestCase):
    def setUp(self):
        self.rows = [
            {"event": "measurement", "correct": True, "schemaVersion": 2,
             "scenario": scenario, "sample": sample, "semanticSha256": scenario,
             "corpus": {"sha256": "fixed", "files": 1000},
             "platform": {"os": "test", "arch": "test"}, "toolchain": "fixed",
             "totalMs": elapsed, "peakProcessRssBytes": 1000, "nodes": 40, "edges": 39,
             "metrics": {"cacheBytes": 500, "filesRead": 0, "linkParses": 0, "yamlParses": 0,
                         "cacheRebuilt": scenario in ("cold", "no-cache"),
                         "phasesMs": {"inventory": elapsed / 2, "readAndParse": 1}}}
            for scenario in sorted(SCENARIOS)
            for sample, elapsed in enumerate((10, 100, 20))
        ]

    def load(self, rows):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "samples.jsonl"
            path.write_text("".join(json.dumps(row) + "\n" for row in rows))
            return samples(path, 3)

    def test_medians_and_optional_rss(self):
        candidate = copy.deepcopy(self.rows)
        for row in candidate:
            row["totalMs"] /= 2
            row["metrics"]["phasesMs"]["inventory"] /= 2
            row["peakProcessRssBytes"] = None
        delta = compare(self.load(self.rows), self.load(candidate))["warm"]
        self.assertEqual(delta["totalMs"], {"baseline": 20, "candidate": 10, "deltaPercent": -50})
        self.assertEqual(delta["peakProcessRssBytes"]["candidate"], None)
        self.assertEqual(delta["filesRead"]["deltaPercent"], None)
        self.assertEqual(delta["phase:inventory"], {"baseline": 10, "candidate": 5, "deltaPercent": -50})

    def test_rejects_missing_or_duplicate_samples(self):
        for rows in (self.rows[:-1], self.rows + self.rows[:1]):
            with self.assertRaisesRegex(ValueError, "Incomplete samples"):
                self.load(rows)
        rows = copy.deepcopy(self.rows)
        rows[0]["sample"] = 1
        with self.assertRaisesRegex(ValueError, "Incomplete samples"):
            self.load(rows)

    def test_focused_comparison_requires_all_explicitly_selected_scenarios(self):
        baseline = self.load(self.rows)
        rows = [row for row in self.rows if row["scenario"] in ("warm", "incremental")]
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "samples.jsonl"
            path.write_text("".join(json.dumps(row) + "\n" for row in rows))
            focused = samples(path, 3, {"warm", "incremental"})
            self.assertEqual(set(compare(baseline, focused)), {"warm", "incremental"})
            with self.assertRaisesRegex(ValueError, "Missing or unexpected"):
                samples(path, 3, {"warm", "incremental", "wide"})
            with self.assertRaisesRegex(ValueError, "missing candidate scenarios"):
                compare(focused, baseline)

    def test_rejects_partial_or_failed_runs(self):
        with self.assertRaisesRegex(ValueError, "Missing or unexpected scenarios"):
            self.load([row for row in self.rows if row["scenario"] != "cold"])
        rows = copy.deepcopy(self.rows)
        rows[0]["correct"] = False
        with self.assertRaisesRegex(ValueError, "Invalid measurement"):
            self.load(rows)

    def test_rejects_unexpected_cache_rebuilds(self):
        rows = copy.deepcopy(self.rows)
        row = next(row for row in rows if row["scenario"] == "rename")
        row["metrics"]["cacheRebuilt"] = True
        with self.assertRaisesRegex(ValueError, "unexpected cache rebuild state"):
            self.load(rows)

    def test_rejects_changed_output_and_inconsistent_samples(self):
        rows = copy.deepcopy(self.rows)
        rows[0]["semanticSha256"] = "changed"
        with self.assertRaisesRegex(ValueError, "Inconsistent output"):
            self.load(rows)
        for row in rows:
            row["semanticSha256"] = "changed"
        with self.assertRaisesRegex(ValueError, "semanticSha256 changed"):
            compare(self.load(self.rows), self.load(rows))

    def test_rejects_different_workloads_and_machines(self):
        for field in ("corpus", "platform", "toolchain", "schemaVersion"):
            rows = copy.deepcopy(self.rows)
            for row in rows:
                row[field] = "different"
            with self.assertRaisesRegex(ValueError, field + " changed"):
                compare(self.load(self.rows), self.load(rows))
        rows[0]["toolchain"] = "inconsistent"
        with self.assertRaisesRegex(ValueError, "different corpora, platforms, or toolchains"):
            self.load(rows)

    def test_retry_preserves_failed_samples_and_requires_identical_binaries(self):
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            cli = directory / "linkrange"
            harness = directory / "harness"
            cli.write_bytes(b"original cli")
            harness.write_bytes(b"original harness")
            info = {"status": "failed", "error": "interrupted", "finished": "earlier",
                    "binarySha256": hashlib.sha256(cli.read_bytes()).hexdigest(),
                    "harnessSha256": hashlib.sha256(harness.read_bytes()).hexdigest()}
            write_json(directory / "run.json", info)
            (directory / "samples.jsonl").write_text("partial evidence\n")
            cli.write_bytes(b"changed cli")
            with self.assertRaisesRegex(ValueError, "missing or changed"):
                prepare_retry(directory, harness)
            self.assertFalse((directory / "attempts").exists())
            cli.write_bytes(b"original cli")
            retried, binary = prepare_retry(directory, harness)
            self.assertEqual(binary, cli)
            self.assertEqual(retried["attempt"], 2)
            self.assertNotIn("error", retried)
            self.assertEqual((directory / "attempts/1/samples.jsonl").read_text(), "partial evidence\n")
            self.assertEqual(json.loads((directory / "attempts/1/run.json").read_text()), info)
            write_json(directory / "run.json", {**retried, "status": "measured"})
            with self.assertRaisesRegex(ValueError, "Only failed"):
                prepare_retry(directory, harness)


if __name__ == "__main__":
    unittest.main()
