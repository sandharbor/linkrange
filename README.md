# Linkrange

A Rust library and CLI for traversing and returning bounded context graphs over
linked files (Markdown, but also HTML, SVG, Excalidraw, and more). Linkrange
resolves links, indexes incoming relationships, and returns the neighborhood you
ask for, with enough provenance info for another tool to be able to use the data.

Use it as a primitive for malleable software: assemble a context bundle for an
agent, inspect a note's neighborhood, preview what lies beyond a traversal
boundary (in the frontier), or feed a publishing pipeline.

[Meadow](https://github.com/sandharbor/meadow)
is the driving integration, but Linkrange stands alone.

## Library and CLI

Build from source with stable Rust:

```sh
cargo install --git https://github.com/sandharbor/linkrange --locked
linkrange query --source notes=/absolute/path/to/notes \
  --start source://notes/Projects --start source://notes/Ideas.md \
  --outlinks 2 --inlinks 1 --frontier-depth 1 --adjacency
```

For reproducible installation add `--rev <full-commit-sha>`. The CLI prints one
JSON response on stdout. Query failures print JSON on stderr and exit nonzero. Complex
requests use `linkrange query --request request.json` (or `-` for stdin):

```json
{
  "sources": [{"name": "notes", "directory": "/absolute/path/to/notes"}],
  "index": {
    "cacheDirectory": "/absolute/path/to/cache",
    "frontmatter": [{"key": "sensitive"}]
  },
  "query": {
    "starts": [{"path": "source://notes/Ideas.md"}, {"path": "source://notes/Projects"}],
    "depths": {"outlinks": 2, "inlinks": 1},
    "rules": [{"path": "source://notes/Archive", "subtree": true, "exclude": true}],
    "frontierDepth": 1,
    "boundaryEmbedTypes": ["png", "svg"],
    "adjacency": true,
    "explainResolution": true
  }
}
```

Responses omit metrics by default. Add `--metrics` to include timing, parsing,
and cache measurements; this also works with `--request`. JSON requests can set
`"metrics": true` inside `query`, and Rust callers can set `Query::metrics` to
`true`. `Response::metrics` is `None` unless requested. `Graph::metrics()` provides
explicit access to indexing measurements when inspecting an open graph.

Every request supplies a nonempty `sources` array, with one entry for each named
directory. Starts, rules, and lookup paths use `source://name/path`, including
when there is only one source. A folder start seeds its supported descendant
files at depth zero. Files and folders may be mixed.

### Multiple sources

An explicit source registry admits separate directories into one graph. Directories
need not be repositories or vaults. Add entries to the same `sources` array:

```json
{
  "sources": [
    {"name": "notes", "directory": "/local/notes"},
    {"name": "research", "directory": "/local/research", "aliases": ["papers"]}
  ],
  "query": {
    "starts": [{"path": "source://notes/Overview.md"}],
    "depths": {"outlinks": 2, "inlinks": 1},
    "rules": [{"path": "source://papers/Archive", "subtree": true, "exclude": true}],
    "adjacency": true
  }
}
```

Every response returns the canonicalized `sources` registry once.
Every path-bearing identity—including file directories, route
steps, alternative routes, edge endpoints, diagnostic paths, adjacency keys and
neighbors, lookup records, and resolution candidates—uses
`source://<canonical-name>/<source-relative-path>`. Paths percent-encode reserved
URL characters and Unicode bytes; a root directory is `source://notes/`.
The public `source_locator` and `parse_source_locator` functions round-trip these
identities. Queries accept aliases but results use canonical names. Unqualified
selectors are rejected, even with one source.

Rust callers use `Graph::open(&sources, &options)` and can inspect
`Graph::sources()`. CLI callers repeat `--source NAME=DIRECTORY` for each source;
use a JSON request to also configure aliases. Combining `--request` with
`--source` fails explicitly.

Names and aliases are case-sensitive and match `[a-z][a-z0-9_-]{0,63}`. Every
spelling must be unique, including aliases within an entry. Physical directories
must be distinct and non-overlapping after filesystem canonicalization. Registry
validation checks all roots before loading caches. A symlink never admits another
source implicitly, even if that directory is separately registered.

### Source-qualified links

Wikilink targets reserve `::` for qualification, before any heading, block, alias,
or image-size suffix: `[[Overview::research#Summary|Read more]]` and
`![[diagram.png::papers|300]]`. Qualification uses the configured name or alias.
`::` in an alias or anchor is ordinary text; literal `::` in a filename can be
addressed through a URL instead of a wikilink.

Markdown and HTML/SVG support `source://research/Overview.md`, including images,
objects, SVG `use`, and the other supported link/embed attributes. URL paths start
at the selected source root, regardless of the referring file's directory. They
are percent-decoded once, after separating queries and fragments. Queries do not
affect file identity; fragments retain the existing heading/block semantics.
Original spelling remains available for rewriting generated links. Empty paths,
malformed escapes, invalid names, backslashes, and parent-directory segments in
source URLs remain unresolved with `invalidSourceReference` diagnostics. Request
locators cannot contain raw queries or fragments: encode literal filename
characters such as `#`, `?`, and `%`.

Unqualified links resolve only within their referring source. Qualified wikilinks
use the existing ranking within the selected source, with root/shallowest/lexical
ranking when crossing sources; they never borrow the origin's directory. An alias
selecting the referring source retains ordinary same-source ranking. A recognized
source with a missing file is distinct from an unknown source: the former has a
selected canonical source and a null target; the latter has `unregisteredSource`
diagnostics and never falls back to a local namesake.

Links expose `link_source_name`, `link_requested_target_source`,
`link_resolved_target_source`, and, on invalid or unknown references,
`link_source_error`. Diagnostics include `requestedSource` and `linkOriginalText`.
Source diagnostics follow the existing query scope: returned and lookup files
contribute occurrences; unrelated inventory files do not. Frontier occurrences
remain inspectable and their referring node's `inclusion` distinguishes them from
normal traversal.

All configured sources are indexed before resolution, including sources reached
only through incoming links. Crossing a source boundary consumes the ordinary
link budgets, preserves overrides and arrival provenance, and does not restart
traversal. Adding a source does not add a start or include its whole inventory.
Caches remain per physical directory: names, aliases, selected source sets, and
starts do not cause unchanged files to be reparsed. Newly appearing targets can
resolve previously cached links. An unavailable configured root fails the request
without turning its inventory into deletions; `bestEffort` still only permits
explicitly diagnosed partial indexing inside available roots.

Add the crate as a Git dependency pinned to a revision, then use the same API:

```rust
use linkrange::{Graph, IndexOptions, Query, Source, Start};

fn main() -> anyhow::Result<()> {
    let sources = vec![Source {
        name: "notes".into(),
        directory: "notes".into(),
        aliases: vec![],
    }];
    let graph = Graph::open(&sources, &IndexOptions::default())?;
    let result = graph.query(&Query {
        starts: vec![Start { path: "source://notes/Ideas.md".into(), depths: None }],
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
  Markdown and HTML/SVG URL paths are percent-decoded once (including `%20`
  spaces) after separating query strings and fragments. Original URLs remain
  in `link_original_text` for consumers that rewrite links; wikilinks keep their
  existing literal filename semantics.
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
  Each route step's `retainedForTraversal` identifies whether that exact arrival
  belongs to the final traversal states or is preserved only as explanation.
  Several arrivals may be retained when their budgets trade off; equal numeric
  budgets alone do not establish which route is retained.
  Independent maxima are useful display summaries,
  never a combined traversal state: arrivals with budgets 5/0 and 2/3 do not create 5/3.
- **Pruning:** `stop` includes a node but prevents expansion through it; `exclude`
  omits it entirely. `subtree` applies a rule to a directory's descendants. An
  independent allowed route can still reach a node beyond a stopped branch.
  Rules are only needed for depth overrides, stops, or exclusions. Reachable
  files need no individual rule; listing a path in a rule does not select it.
- **Frontier:** `frontierDepth` extends the returned graph beyond its normal
  boundary. Overrides on frontier-only nodes are ignored, including zero
  overrides. The extension cannot revive exhausted incoming traversal. Stop and
  exclude policies still apply. `inclusion` distinguishes `traversal`, `frontier`,
  and the optional terminal `embeddedAsset` exception.
- **Boundary embeds:** Caller-selected formats directly embedded at a normal
  boundary may be included without expanding through the asset. Ordinary links
  do not receive this exception. `boundaryEmbedTypes` selects target formats;
  `boundaryEmbedSourceTypes` selects source formats whose resolved direct embeds receive
  the exception regardless of target format. For example, `["html"]` preserves
  a boundary HTML page's scripts, stylesheets, images, and embedded documents.
  The CLI exposes this as `--boundary-embed-source-type html`. Both selections
  default to empty; stop and exclude policies still apply.
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
traversal regressions are retained. `tests/fixtures/` contains source graphs,
queries, and expected results. `tests/query_fixtures.rs` checks graph membership,
frontier depths, and links through the public Rust API. Tests use the committed
fixtures. Per-file expectations are authored and checked independently of the
output snapshots.

Each `<fixture>.query.json` defines one query, its `sources`, the
`fixtureDirectory` containing its authored node specifications, and the expected
number of those specifications (`null` when there are none).
Node expectations sit beside the source file, for example
`t002 ---- dup.md.nodespec-big.json` and `t002 ---- dup.md.nodespec-small.json`.
The source extension is retained to distinguish files with the same stem.
The test runner derives the node path from the sidecar filename and checks that
every specification is used. JSON sidecars are not indexed as graph nodes.

`tests/fixtures/expected_outputs/<fixture>.json` contains the expected response
for each query, including queries without per-file expectations. These snapshots
record the complete ordinary response. Fixture queries leave metrics disabled,
so the comparison removes no fields. The fixture test compares JSON values and
checks all the per-file expectations as well. Tests never rewrite expected outputs.

If a snapshot is missing or differs, the test fails and saves the actual response
to `target/fixture_outputs/<fixture>.json`. Compare the files to investigate the
change. To accept an intentional change, review the actual output and copy it
over the corresponding expected output, then rerun the test. A snapshot update
does not bypass the per-file assertions.

See [perf/](perf/README.md) for the performance harness, experiment runner, and
500,000-file diagnostic. Large performance runs are deliberately outside the
curated suite and ordinary `cargo test`.

Apache-2.0.
