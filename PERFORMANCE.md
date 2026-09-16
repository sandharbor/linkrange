# Performance diagnostic and agent experiments

Build the harness once in release mode. Generate the corpus in a new directory
under the OS temporary directory; the generator refuses to overwrite a directory.
It writes exactly 500,000 source files by default, plus its manifest outside the
source root. Corpus creation is not part of the measured query time.

```sh
cargo build --locked --release --example performance
./target/release/examples/performance generate --directory /tmp/linkrange-corpus
./target/release/examples/performance measure --directory /tmp/linkrange-corpus \
  --repeats 3 > results.jsonl
```

Use an unused directory name, or a smaller `--files 1000` for a harness smoke
check. Delete only a corpus you created when done. The corpus has three link
syntaxes, a deterministic ternary graph, variable prose lengths, selected and
irrelevant frontmatter, malformed irrelevant YAML, and ignored code/comments.
The manifest contains a generator version, file count, byte count, seed, and
content fingerprint. This synthetic workload complements the authored suite;
it does not claim to model every real vault.

Each JSON line is independently interpretable: schema version, scenario, sample,
Git revision, corpus identity, platform, total/open/serialization milliseconds,
phase timings, reads/link parses/YAML parses, cache bytes, process peak RSS,
response bytes, graph sizes, semantic SHA-256, and correctness status. Peak RSS
is the process lifetime high-water mark, so compare isolated scenario processes
when assessing memory. Timing uses monotonic clocks. Cold means rebuilt index;
OS filesystem caches are not flushed and must be reported as such.

Scenarios: `cold`, `warm` (persisted-cache restart), `incremental` (one content
edit), `metadata-change`, `no-cache`, and `wide`. Default queries stop at three
hops (40 files); `wide` separately uses eight hops (9,841 files on the full
corpus). Corpus size and traversal size are independent. Run one scenario with
`--scenario warm`. Setup/priming and restoring controlled edits are excluded.
The harness validates the expected node count and completeness; semantic hashes
must also agree against the fixed baseline for the same scenario.

## Research protocol

Inspired by [autoresearch](https://github.com/karpathy/autoresearch): make one
bounded change, measure it, retain or revert it, and record the evidence.

1. Finish the initial extraction and pass Rust, Meadow quickcheck, and full E2E
   gates before the optimization phase.
2. Generate the full corpus once. Save baseline source revision, compiler,
   machine conditions, three samples per scenario, semantic hashes, and tests.
3. For each of 25 experiments, record the hypothesis and exact diff before
   measuring. Keep corpus generation, authored expectations, and correctness
   checks fixed. Do not optimize by removing supported syntax or diagnostics.
4. Run Rust correctness tests. Compare same-scenario response hashes, read/parse
   counts, cache sizes, and three-sample medians on the full corpus. Interleave
   the retained baseline when a result is near measurement noise.
5. Favor lower warm/incremental latency without material cold/query/memory
   regressions. An inconclusive change is reverted. Record a failure rather
   than hiding a failed build, changed output, or slower result.
6. Write an append-only JSONL ledger: iteration, hypothesis, base/candidate
   revisions or diff hash, correctness, samples, median deltas, memory/cache
   deltas, decision (`keep`/`revert`), and rationale. Preserve raw run logs.
7. After 25 experiments, rerun the affected integration gates and summarize
   baseline versus final results and limitations. No claim of improvement
   without measurements.

The initial frontmatter delimiter/substring gates are baseline behavior. Possible
experiments include faster link prefilters/tokenization, fewer copies, compact
cache representation, inventory syscall reductions, and indexed resolution.
Optimizing changes to the requested field set is optional later work.
