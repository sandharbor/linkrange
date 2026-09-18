#!/usr/bin/env python3
"""Run and record controlled Linkrange performance experiments (Python 3.9+)."""
import argparse
from collections import defaultdict
from contextlib import contextmanager
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import statistics
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
SCENARIOS = {"cold", "warm", "incremental", "add", "delete", "rename", "metadata-change", "no-cache", "wide"}


def now():
    return datetime.now(timezone.utc).isoformat()


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def read_json(path):
    return json.loads(path.read_text())


def capture(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def logged(args, log, stdout=None):
    print("+ " + " ".join(map(str, args)), flush=True)
    with log.open("ab") as stream:
        subprocess.run(args, cwd=ROOT, stdout=stdout or stream, stderr=stream, check=True)


def samples(path, repeats, scenarios=SCENARIOS):
    groups = defaultdict(list)
    for line in path.read_text().splitlines():
        row = json.loads(line)
        if row.get("event") != "measurement" or row.get("correct") is not True:
            raise ValueError(f"Invalid measurement in {path}")
        groups[row["scenario"]].append(row)
    if set(groups) != set(scenarios):
        raise ValueError(f"Missing or unexpected scenarios in {path}")
    identity = None
    for scenario, rows in groups.items():
        if len(rows) != repeats or sorted(row["sample"] for row in rows) != list(range(repeats)):
            raise ValueError(f"Incomplete samples for {scenario}")
        if len({row["semanticSha256"] for row in rows}) != 1:
            raise ValueError(f"Inconsistent output for {scenario}")
        for row in rows:
            current = [row["schemaVersion"], row["corpus"], row["platform"], row["toolchain"]]
            if identity is not None and current != identity:
                raise ValueError("Measurements use different corpora, platforms, or toolchains")
            identity = current
            if not isinstance(row.get("metrics"), dict) or row["totalMs"] <= 0:
                raise ValueError("Measurement is missing metrics or a valid elapsed time")
            rebuilt = scenario in ("cold", "no-cache")
            if row["metrics"].get("cacheRebuilt") is not rebuilt:
                raise ValueError(f"{scenario}: unexpected cache rebuild state; comparison rejected")
    return groups


def compare(baseline, candidate):
    result = {}
    if not set(candidate) <= set(baseline):
        raise ValueError("Comparison baseline is missing candidate scenarios")
    for scenario in sorted(candidate):
        before, after = baseline[scenario], candidate[scenario]
        for field in ("schemaVersion", "corpus", "platform", "toolchain", "semanticSha256", "nodes", "edges"):
            if before[0][field] != after[0][field]:
                raise ValueError(f"{scenario}: {field} changed; comparison rejected")
        def median(rows, key):
            if key.startswith("phase:"):
                values = [row["metrics"]["phasesMs"].get(key[6:]) for row in rows]
            else:
                values = [row.get(key, row["metrics"].get(key)) for row in rows]
            return None if any(value is None for value in values) else statistics.median(values)
        deltas = {}
        keys = ["totalMs", "peakProcessRssBytes", "cacheBytes", "filesRead", "linkParses", "yamlParses"]
        keys.extend("phase:" + phase for phase in sorted(before[0]["metrics"]["phasesMs"]))
        for key in keys:
            old, new = median(before, key), median(after, key)
            deltas[key] = {"baseline": old, "candidate": new,
                           "deltaPercent": (new / old - 1) * 100 if old and new is not None else None}
        result[scenario] = deltas
    return result


def display(comparison):
    print("\nScenario           baseline ms  candidate ms   change     scan before ms  scan after ms")
    for scenario, values in comparison.items():
        timing = values["totalMs"]
        scan = values["phase:inventory"]
        print(f"{scenario:18} {timing['baseline']:11.2f} {timing['candidate']:13.2f} {timing['deltaPercent']:+7.1f}%"
              f" {scan['baseline']:17.2f} {scan['candidate']:14.2f}")
    print("Scan is the filesystem inventory; changed-file reads/parsing are separate.")
    print("Negative changes mean faster. Read comparison.json for all phases, memory, cache, and parse counts.")


def ledger(session, entry):
    with (session / "experiments.jsonl").open("a") as stream:
        stream.write(json.dumps({"time": now(), **entry}) + "\n")
        stream.flush()
        os.fsync(stream.fileno())


def entries(session):
    path = session / "experiments.jsonl"
    return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []


def load_samples(session, label):
    info = read_json(session / label / "run.json")
    return samples(session / label / "samples.jsonl", read_json(session / "session.json")["repeats"],
                   info.get("scenarios", SCENARIOS))


@contextmanager
def exclusive(session):
    lock = session / ".running"
    lock.mkdir()  # Refuse overlapping measurements of the same corpus.
    try:
        yield
    finally:
        lock.rmdir()


def provenance(destination):
    (destination / "source.diff").write_text(capture("git", "diff", "--binary", "HEAD") + "\n")
    untracked = capture("git", "ls-files", "--others", "--exclude-standard", "-z")
    for name in filter(None, untracked.split("\0")):
        target = destination / "untracked" / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / name, target)
    return {"revision": capture("git", "rev-parse", "HEAD"),
            "diffSha256": hashlib.sha256((destination / "source.diff").read_bytes()).hexdigest(),
            "worktreeStatus": capture("git", "status", "--porcelain"),
            "toolchain": capture("rustc", "--version"), "machine": platform.platform()}


def build(destination):
    log = destination / "checks.log"
    logged(["cargo", "fmt", "--check"], log)
    logged(["cargo", "clippy", "--locked", "--all-targets", "--", "-D", "warnings"], log)
    logged(["cargo", "test", "--locked"], log)
    logged([sys.executable, "-m", "unittest", "discover", "-s", "perf", "-p", "test_*.py"], log)
    # Isolate release binaries from other Cargo target-directory settings.
    target = ROOT / "target" / "perf-build"
    logged(["cargo", "build", "--locked", "--release", "--target-dir", str(target),
            "--bin", "linkrange", "--example", "performance"], log)
    suffix = ".exe" if os.name == "nt" else ""
    cli = destination / ("linkrange" + suffix)
    shutil.copy2(target / "release" / ("linkrange" + suffix), cli)
    return cli, target / "release" / "examples" / ("performance" + suffix)


def prepare_retry(destination, harness):
    info = read_json(destination / "run.json")
    if info["status"] != "failed":
        raise ValueError("Only failed measurements can be retried")
    cli = destination / ("linkrange.exe" if os.name == "nt" else "linkrange")
    for path, key in ((cli, "binarySha256"), (harness, "harnessSha256")):
        if not path.exists() or hashlib.sha256(path.read_bytes()).hexdigest() != info.get(key):
            raise ValueError(f"Cannot retry: missing or changed {path.name}; start a new experiment")
    attempt = info.get("attempt", 1)
    archive = destination / "attempts" / str(attempt)
    archive.mkdir(parents=True)
    for name in ("run.json", "samples.jsonl", "stderr.log", "comparison.json"):
        path = destination / name
        if path.exists():
            shutil.move(path, archive / name)
    info.update(attempt=attempt + 1, started=now(), status="running")
    info.pop("finished", None)
    info.pop("error", None)
    return info, cli


def measure(session, label, hypothesis=None, baseline_label=None, retry=False, scenarios=None, kind="experiment"):
    config = read_json(session / "session.json")
    destination = session / label
    harness = session / ("harness.exe" if os.name == "nt" else "harness")
    if retry:
        info, cli = prepare_retry(destination, harness)
        baseline_label = info["baseline"]
    else:
        destination.mkdir()
        info = {"label": label, "hypothesis": hypothesis, "baseline": baseline_label,
                "started": now(), "status": "running", "attempt": 1,
                "scenarios": scenarios or sorted(SCENARIOS), "kind": kind}
    write_json(destination / "run.json", info)
    try:
        if not retry:
            info.update(provenance(destination))
            cli, built_harness = build(destination)
            info["binarySha256"] = hashlib.sha256(cli.read_bytes()).hexdigest()
            write_json(destination / "run.json", info)
        if baseline_label is None and not retry:
            shutil.copy2(built_harness, harness)
            with (destination / "generation.jsonl").open("w") as output:
                logged([str(harness), "generate", "--directory", str(session / "corpus"),
                        "--files", str(config["files"])], destination / "stderr.log", output)
        info["harnessSha256"] = hashlib.sha256(harness.read_bytes()).hexdigest()
        write_json(destination / "run.json", info)
        # Every candidate uses the original harness and corpus.
        with (destination / "samples.jsonl").open("w") as output:
            selected = info.get("scenarios", SCENARIOS)
            for scenario in (["all"] if set(selected) == SCENARIOS else selected):
                logged([str(harness), "measure", "--directory", str(session / "corpus"),
                        "--repeats", str(config["repeats"]), "--cli", str(cli),
                        "--scenario", scenario], destination / "stderr.log", output)
        candidate = load_samples(session, label)
        if baseline_label is not None:
            original = load_samples(session, "baseline")
            compare(original, candidate)  # Keep the initial correctness reference fixed.
            baseline = load_samples(session, baseline_label)
            comparison = compare(baseline, candidate)
            write_json(destination / "comparison.json", comparison)
            display(comparison)
        info["status"] = "measured"
    except (Exception, KeyboardInterrupt) as error:
        info["status"] = "failed"
        info["error"] = str(error) or type(error).__name__
        raise
    finally:
        info["finished"] = now()
        write_json(destination / "run.json", info)
        ledger(session, info)
    print(f"\nSaved {destination}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    start = commands.add_parser("start", help="Check, build, generate a corpus, and measure the baseline")
    start.add_argument("--session", type=Path, required=True, help="New directory under perf/results/ or outside the checkout")
    start.add_argument("--files", type=int, default=500_000)
    start.add_argument("--repeats", type=int, default=3)
    start.add_argument("--experiments", type=int, default=10)
    run = commands.add_parser("experiment", help="Check and measure the current code against the last retained run")
    run.add_argument("--session", type=Path, required=True)
    run.add_argument("--label", required=True)
    run.add_argument("--hypothesis", required=True)
    run.add_argument("--scenarios", nargs="+", choices=sorted(SCENARIOS),
                     help="Measure a focused subset; default: all nine scenarios")
    retry = commands.add_parser("retry", help="Retry a failed measurement using its frozen binary; preserve prior attempts")
    retry.add_argument("--session", type=Path, required=True)
    retry.add_argument("--label", required=True)
    validate = commands.add_parser("validate", help="Check current code and measure all scenarios against the original baseline")
    validate.add_argument("--session", type=Path, required=True)
    validate.add_argument("--label", default="final-validation")
    record = commands.add_parser("record", help="Record a decision; does not edit or revert source code")
    record.add_argument("--session", type=Path, required=True)
    record.add_argument("--label", required=True)
    record.add_argument("--decision", choices=("keep", "revert"), required=True)
    record.add_argument("--reason", required=True)
    report = commands.add_parser("report", help="Show decisions and compare the final retained run with the initial baseline")
    report.add_argument("--session", type=Path, required=True)
    args = parser.parse_args()
    session = args.session.resolve()
    if session == ROOT or (ROOT in session.parents and ROOT / "perf" / "results" not in session.parents):
        parser.error("Use a session directory under perf/results/ or outside the checkout")
    if args.command == "start":
        if args.files < 100 or args.repeats < 1 or args.experiments < 1:
            parser.error("Use at least 100 files, one sample, and one experiment")
        session.mkdir(parents=True, exist_ok=False)
        write_json(session / "session.json", {"schemaVersion": 1, "files": args.files,
                   "repeats": args.repeats, "experiments": args.experiments, "created": now()})
        with exclusive(session):
            measure(session, "baseline", "Initial baseline", None)
        return
    if args.command == "report":
        history = entries(session)
        config = read_json(session / "session.json")
        decisions = [row for row in history if row.get("decision")]
        print(f"{len(decisions)}/{config['experiments']} experiment decisions recorded")
        for row in decisions:
            print(f"{row['label']}: {row['decision']} — {row['reason']}")
        final = next((row["label"] for row in reversed(decisions) if row["decision"] == "keep"), "baseline")
        print(f"\nInitial baseline versus {final}")
        display(compare(load_samples(session, "baseline"), load_samples(session, final)))
        for row in history:
            if row.get("kind") == "validation" and row.get("status") == "measured":
                print(f"\nFull validation versus initial baseline: {row['label']}")
                display(compare(load_samples(session, "baseline"), load_samples(session, row["label"])))
        return
    if not re.fullmatch(r"[a-z0-9][a-z0-9-]*", args.label) or args.label == "baseline":
        parser.error("Experiment labels must use lowercase letters, numbers, and hyphens; baseline is reserved")
    with exclusive(session):
        history = entries(session)
        if args.command == "record":
            info = read_json(session / args.label / "run.json")
            if info.get("kind") == "validation":
                parser.error("Validation runs do not receive experiment decisions")
            if any(row.get("decision") and row["label"] == args.label for row in history):
                parser.error("This experiment already has a decision")
            if info["status"] != "measured" and args.decision == "keep":
                parser.error("Cannot keep a failed experiment")
            ledger(session, {"label": args.label, "decision": args.decision, "reason": args.reason})
            print(f"Recorded {args.decision}: {args.label}. Source code is unchanged by this command.")
        elif args.command == "retry":
            if any(row.get("decision") and row["label"] == args.label for row in history):
                parser.error("Cannot retry an experiment after recording its decision")
            measure(session, args.label, retry=True)
        elif args.command == "validate":
            measure(session, args.label, "Full scenario validation", "baseline", kind="validation")
        else:
            if args.scenarios and len(args.scenarios) != len(set(args.scenarios)):
                parser.error("Each scenario must appear only once")
            decided = {row["label"] for row in history if row.get("decision")}
            attempted = {row["label"] for row in history if row["label"] != "baseline"
                         and "status" in row and row.get("kind") != "validation"}
            if attempted - decided:
                parser.error("Record the previous experiment's decision before starting another")
            if len(attempted) >= read_json(session / "session.json")["experiments"]:
                parser.error("This session has reached its requested experiment count")
            baseline = next((row["label"] for row in reversed(history) if row.get("decision") == "keep"), "baseline")
            available = set(read_json(session / baseline / "run.json").get("scenarios", SCENARIOS))
            if not set(args.scenarios or SCENARIOS) <= available:
                parser.error("Retained baseline lacks some requested scenarios; use validate for a full comparison")
            measure(session, args.label, args.hypothesis, baseline, scenarios=args.scenarios)


if __name__ == "__main__":
    try:
        main()
    except (Exception, KeyboardInterrupt) as error:
        print(f"Performance run failed: {error}. See the session's run.json and logs.", file=sys.stderr)
        sys.exit(1)
