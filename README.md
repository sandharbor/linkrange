# Linkrange

A Rust library and CLI for bounded context graphs over linked files. Linkrange
resolves links, indexes incoming relationships, and returns the neighborhood you
ask for, with enough provenance for another tool to use the same resolutions.

Use it as a primitive for malleable software: assemble a context bundle for an
agent, inspect a note's neighborhood, preview what lies beyond a traversal
boundary, or feed a publishing pipeline. [Meadow](https://github.com/sandharbor/meadow)
is the driving integration. Linkrange has no Meadow application configuration.

## Library and CLI

Build from source with stable Rust:

```sh
cargo install --git https://github.com/sandharbor/linkrange --locked
linkrange query --source-root ~/notes --start Projects --start Ideas.md \
  --outlinks 2 --inlinks 1 --frontier-depth 1 --adjacency
```

For reproducible installation add `--rev <full-commit-sha>`. The CLI prints one
JSON response on stdout. Errors print JSON on stderr and exit nonzero. Complex
requests use `linkrange query --request request.json` (or `-` for stdin):

```json
{
  "sourceRoot": "/absolute/path/to/notes",
  "index": {
    "cacheDirectory": "/absolute/path/to/cache",
    "frontmatter": [{"key": "sensitive"}]
  },
  "query": {
    "starts": [{"path": "Ideas.md"}, {"path": "Projects"}],
    "depths": {"outlinks": 2, "inlinks": 1},
    "rules": [{"path": "Archive", "subtree": true, "exclude": true}],
    "frontierDepth": 1,
    "boundaryEmbedTypes": ["png", "svg"],
    "adjacency": true,
    "explainResolution": true
  }
}
```

All paths in a query and response are physical source-root-relative paths using
`/`. A folder start seeds its supported descendant files at depth zero. Files and
folders may be mixed; no virtual collection node is inserted. Detected formats
are separate from paths: `drawing.excalidraw.md` remains its physical path.

Add the crate as a Git dependency pinned to a revision, then use the same API:

```rust
use linkrange::{Graph, IndexOptions, Query, Start};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let graph = Graph::open(Path::new("notes"), &IndexOptions::default())?;
    let result = graph.query(&Query {
        starts: vec![Start { path: "Ideas.md".into(), depths: None }],
        adjacency: true,
        ..Query::default()
    })?;
    println!("{} files", result.nodes.len());
    Ok(())
}
```

`linkrange::query(&Request)` opens the index and executes one query.
`Graph::open` followed by repeated `Graph::query` calls reuses an in-memory graph.
Reopen to observe filesystem changes. Rustdoc exposes the request/response types.

## Behavioral contract

- **Links:** Markdown inline/reference links and images, Obsidian wikilinks with
  aliases/headings/blocks, Excalidraw links, native HTML and SVG, and HTML markup
  inside Markdown. Markdown code examples and HTML comments do not create links.
  Obsidian-style inline destinations containing spaces are accepted.
- **HTML/SVG:** Element-aware `href`, `xlink:href`, `src`, `poster`, and object
  `data` attributes distinguish anchors from embedded resources. SVG `image`
  and `use` references are embeds. Script/style text isn't scanned for markup.
  This is local-file discovery, not a browser dependency crawler: CSS URLs,
  JavaScript imports, `srcset`, and HTML `base` URL semantics are not implemented.
  Remote URLs and same-file HTML fragments are not graph edges.
- **Resolution:** Preserve the empirically observed Obsidian rules: exact path
  where specified; otherwise root match, then linking-file directory, then
  shallowest candidate, then lexical order. Explicit directories also support
  suffix matching. `explainResolution` includes the winning reason and candidates.
- **Budgets:** Every traversed link decrements both remaining outgoing and incoming
  budgets; incoming clamps at zero. Incoming links additionally need incoming
  budget. Per-node overrides reset each independently when reached inside the
  normal boundary. Multiple routes retain useful independent budget states;
  node display details describe a shortest valid arrival, not a synthetic merge.
  Each node's `routeSteps` records the actual depth, direction, inherited and
  overridden budgets, and inclusion at every step of its `route`. An intermediate
  page's own shortest arrival may have different budgets; use `routeSteps` to
  explain why traversal could continue along the selected route. `alternativeRoutes`
  supplies the other non-dominated arrivals with their complete steps, including
  intermediate budget tradeoffs. It also preserves one route for the strongest
  inherited budget in each overridden direction, even when overrides make those
  routes redundant for traversal. This explains reductions such as 1 → 0 without
  reviving traversal; `states` still contains only useful post-override budgets.
  Independent maxima are useful display summaries,
  never a combined traversal state: arrivals with budgets 5/0 and 2/3 do not create 5/3.
- **Pruning:** `stop` includes a node but prevents expansion through it; `exclude`
  omits it entirely. `subtree` applies a rule to a directory's descendants. An
  independent allowed route can still reach a node beyond a stopped branch.
- **Frontier:** `frontierDepth` extends the returned graph beyond its normal
  boundary. Overrides on frontier-only nodes are ignored, including zero
  overrides. The extension cannot revive exhausted incoming traversal. Stop and
  exclude policies still apply. `inclusion` distinguishes `traversal`, `frontier`,
  and the optional terminal `embeddedAsset` exception.
- **Boundary embeds:** Caller-selected formats directly embedded at a normal
  boundary may be included without expanding through the asset. Ordinary links
  do not receive this exception.
- **Query scope:** `nodes`, `edges`, and `linksBySource` describe the requested
  result, not the whole vault. For each returned source, `linksBySource` includes
  every parsed outgoing occurrence, even when its target is outside the result.
  `target: null` means unresolved. The `link_original_text` and parsed fields
  retain resolver provenance. Optional `lookupPaths` requests additional source
  link records without including those files as nodes.
- **Adjacency:** Optional `adjacency` returns all incoming/outgoing neighbors of
  returned/lookup files, including outside neighbors when incoming traversal is
  zero. Those neighbors are not implicitly added to the result.
- **Frontmatter:** Only requested keys are returned, with JSON value types. A
  leading `---` and at least one requested literal substring in the candidate
  header are required before YAML parsing; `substring` defaults to the key.
  Unrelated malformed YAML is ignored. Relevant malformed headers produce
  per-file diagnostics and the caller decides how to handle them. Empty
  selections never parse YAML. Missing keys remain absent.
- **Filesystem failures:** Indexing is strict by default and preserves the prior
  cache on failure. `bestEffort: true` explicitly permits partial results with
  `complete: false` and diagnostics. Relevant frontmatter diagnostics are
  query-scoped; filesystem diagnostics apply to the inventory.
- **Symlinks:** Default is skip and report. `symlinks: "followInternal"` (CLI
  `--follow-internal-symlinks`) follows only targets inside the root, deduplicates
  aliases, detects directory cycles, and cannot bypass query exclusions or the
  hidden-file policy. Outside targets are reported and skipped.

## Incremental cache

The cache holds parsed links, file identities/digests, and only the requested
metadata values, absences, and diagnostics. Changed field selections can reread
metadata while reusing parsed links. It does not retain raw frontmatter or index
every possible field. Warm queries still inventory the source tree so additions,
removals, and changed ambiguous targets are visible.

Use `--cache-directory` outside the source root, `--no-cache`, or `--rebuild`.
The default is `linkrange-cache` in the OS temporary directory. Cache corruption
or parser-version changes cause a rebuild. Writers serialize under a file lock
and publish by rename. On Unix, size, modification time, change time, device,
and inode detect changes (including same-size writes with restored mtime).
Other platforms conservatively reread files. The first version is tested on
macOS and Linux. Indexing requires a stable source tree and reports concurrent
changes so the caller can retry; it is not a filesystem snapshot service.

## Executable specifications

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

Focused Rust scenarios assert each promised behavior through the API and CLI,
using real temporary files for filesystem/cache/symlink cases. Mature parser and
traversal regressions are retained. `fixtures/generated` is a fully replaceable
export of public Meadow source graphs, authored sourcing expectations, and
portable queries derived from home fixtures. Its manifest records input hashes.
Tests use the committed snapshot and need no Meadow checkout or Node runtime.
Expected answers are authored; the engine never regenerates them from its own
output. Meadow's curation/generation specifications remain with Meadow.

See [PERFORMANCE.md](PERFORMANCE.md) for the separate 500,000-file diagnostic and
agent experiment protocol. Large performance runs are deliberately outside the
curated suite and ordinary `cargo test`.

Apache-2.0. Original parser and traversal work extracted from Meadow.
