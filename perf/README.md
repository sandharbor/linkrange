# Performance testing

This folder contains the tools and protocol for measured optimization experiments:

- `harness.rs`: deterministic corpus generation and process-level CLI measurements.
- `run.py`: release builds, correctness gates, baseline comparisons, and an experiment ledger.
- `test_runner.py`: tests for rejecting incomplete runs and changed outputs.

Store run outputs and reports under `perf/results/`, which is ignored by Git.

## Start a ten-experiment session

Requires Rust, Git, and Python 3.9 or newer. From the Linkrange directory:

```sh
python3 perf/run.py start --session perf/results/ten-experiments --experiments 10
```

This checks formatting, lint, and tests, builds release binaries, generates
500,000 files, and records three samples for each of nine scenarios. A session
must use a new directory. Build and corpus generation time are excluded from
measurements. Allow several minutes or more for a full run; keep the machine's
load and power conditions consistent.

An experiment is one proposed code change. The runner measures the code you
have edited; it does not invent optimizations or apply/revert changes. After
making a candidate change:

```sh
python3 perf/run.py experiment --session perf/results/ten-experiments \
  --label 01-fewer-copies --hypothesis 'Avoid repeated path allocations during indexing'
```

The runner repeats the checks and measurements, then reports per-scenario median
time changes against the most recently retained run (initially the baseline).
Filesystem scan time appears separately, including for the one-file edit, add,
delete, and rename scenarios. `phase:inventory` measures walking the source tree
and collecting file stamps; `phase:readAndParse` includes comparing those stamps
with cached records and reading/parsing changed files. An incremental run still
inventories the entire corpus, even when it reads only one file's contents.
All response hashes must also match the original baseline. Corpus identity,
platform, toolchain, sample counts, and within-scenario consistency are checked.
Cached scenarios must actually reuse a cache; an unexpected rebuild fails the
run so a cold sample cannot be silently counted as an incremental measurement.
Each session preserves its original harness executable so an optimization cannot
silently change what is measured.

For focused experiments, select the same scenarios for each candidate:

```sh
python3 perf/run.py experiment --session perf/results/ten-experiments \
  --label 02-resolution --hypothesis 'Reduce repeated resolution work' \
  --scenarios warm incremental wide
```

This still collects three samples per selected scenario and checks every output
against the original baseline. It reports only the scenarios measured. Once a
focused candidate is retained, later experiments must use scenarios covered by
that retained run. Finish with a complete validation of the current code:

```sh
python3 perf/run.py validate --session perf/results/ten-experiments
```

Validation repeats all checks and all nine scenarios against the original
baseline. It is recorded separately and does not count as an optimization
experiment. Investigate or revert any regression it reveals before calling the
session complete.

Review `comparison.json`, the raw samples, and the diff. Record the decision:

```sh
python3 perf/run.py record --session perf/results/ten-experiments \
  --label 01-fewer-copies --decision keep --reason 'Consistent warm-cache improvement; outputs unchanged'
```

Use `--decision revert` for regressions, failures, or inconclusive results, then
revert that candidate's code before the next experiment. The record command
only appends a decision; it never changes source files. Labels are unique, and
the runner requires a decision before the next experiment. Failed builds and
measurements remain in the ledger and count as attempted experiments.

To retry an interrupted or invalid measurement without changing its code:

```sh
python3 perf/run.py retry --session perf/results/ten-experiments --label 01-fewer-copies
```

This uses the already checked, frozen CLI and harness, verifies their hashes, and
archives the failed run under `attempts/` before collecting a complete new batch.
Every attempt stays in the ledger; retries count as the same experiment. Record
recurring failures in the results rather than presenting retries as a fix.

Review the decisions and compare the final retained run with the initial baseline:

```sh
python3 perf/run.py report --session perf/results/ten-experiments
```

The session directory contains `session.json`, `experiments.jsonl`, the corpus,
a fixed harness, and a directory for each run. Each run stores `run.json`,
`source.diff`, copies of untracked source files, its CLI binary and SHA-256,
`checks.log`, `stderr.log`, and raw `samples.jsonl`. Candidates also have
`comparison.json` with median total and phase timings, peak memory, cache sizes,
and parse counts. `cacheWrite` also includes a second inventory that checks the
source stayed consistent before publishing an updated cache. `linkParsing` and
`frontmatter` are portions of `readAndParse`, so the phase times are not all additive.
Partial measurements are preserved on failure and cannot receive a keep decision.
Session outputs under `perf/results/` are ignored by Git. Run sessions sequentially;
concurrent workloads distort timing. An interrupted process may leave a
`.running` directory; remove it only after confirming no run is still active.

For a quick tooling check, use a separate small session:

```sh
python3 perf/run.py start --session perf/results/smoke --files 1000 --repeats 1
python3 -m unittest discover -s perf -p 'test_*.py'
```

## Run the measurement harness directly

Build the harness once in release mode. Generate the corpus in a new directory
under `perf/results/`; the generator refuses to overwrite a directory.
It writes exactly 500,000 source files by default, plus its manifest outside the
source root. Corpus creation is not part of the measured query time.

```sh
cargo build --locked --release --bin linkrange --example performance
mkdir -p perf/results
./target/release/examples/performance generate --directory perf/results/manual
./target/release/examples/performance measure --directory perf/results/manual \
  --repeats 3 > perf/results/manual/samples.jsonl
```

Use an unused directory name, or a smaller `--files 1000` for a harness smoke
check. Delete only a corpus you created when done. The corpus has three link
syntaxes, a deterministic ternary graph, ambiguous titles, HTML, SVG, Excalidraw,
leaf assets, variable prose lengths, selected and irrelevant frontmatter,
malformed irrelevant YAML, and ignored code/comments. Generator flags configure
branching, directory size, frontmatter frequency, prose length, and seed.
The manifest contains a generator version, file count, byte count, seed, and
content fingerprint. This synthetic workload complements the authored suite;
it does not claim to model every real vault.

Each JSON line is independently interpretable: schema version, scenario, sample,
Git revision, Rust toolchain, corpus identity, platform, total milliseconds,
phase timings, reads/link parses/YAML parses, cache bytes, process peak RSS,
response bytes, graph sizes, semantic SHA-256, and correctness status. Each sample executes a fresh CLI process: total time includes process startup,
indexing, query, JSON serialization, and output transfer. Peak RSS comes from
`/usr/bin/time` for that process (null when unavailable). Timing uses monotonic clocks. Cold means rebuilt index;
OS filesystem caches are not flushed and must be reported as such.
The harness explicitly requests query metrics. Ordinary CLI responses omit them;
use `--metrics` when collecting measurements outside the harness.

Scenarios: `cold`, `warm` (persisted-cache restart), `incremental` (one content
edit), `add`, `delete`, `rename`, `metadata-change`, `no-cache`, and `wide`. Default queries stop at three
hops (40 files); `wide` separately uses eight hops (9,841 files on the full
corpus). Corpus size and traversal size are independent. Run one scenario with
`--scenario warm`. Setup/priming and restoring controlled edits are excluded.
The harness validates the expected node count and completeness; semantic hashes
must also agree against the fixed baseline for the same scenario.

## Research protocol

Inspired by [autoresearch](https://github.com/karpathy/autoresearch): make one
bounded change, measure it, retain or revert it, and record the evidence.

1. Run `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`,
   and `cargo test --locked` before the optimization phase.
2. Generate the full corpus once. Save baseline source revision, compiler,
   machine conditions, three samples per scenario, semantic hashes, and tests.
3. For each experiment (ten by default), record the hypothesis and exact diff before
   measuring. Keep corpus generation, authored expectations, and correctness
   checks fixed. Do not optimize by removing supported syntax or diagnostics.
4. Run Rust correctness tests. Compare same-scenario response hashes, read/parse
   counts, cache sizes, and three-sample medians on the full corpus. Focused
   experiments may select relevant scenarios, followed by full validation of
   the retained code. Interleave
   the retained baseline when a result is near measurement noise.
5. Favor lower warm/incremental latency without material cold/query/memory
   regressions. An inconclusive change is reverted. Record a failure rather
   than hiding a failed build, changed output, or slower result.
6. Write an append-only JSONL ledger: iteration, hypothesis, base/candidate
   revisions or diff hash, correctness, samples, median deltas, memory/cache
   deltas, decision (`keep`/`revert`), and rationale. Preserve raw run logs.
7. After the planned experiments, rerun the affected integration gates and summarize
   baseline versus final results and limitations. No claim of improvement
   without measurements.

The initial frontmatter delimiter/substring gates are baseline behavior. Possible
experiments include faster link prefilters/tokenization, fewer copies, compact
cache representation, inventory syscall reductions, and indexed resolution.
Optimizing changes to the requested field set is optional later work.
