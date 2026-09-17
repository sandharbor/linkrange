//! Executable behavior specifications. Each test creates real source files and
//! exercises the public library; CLI scenarios use the compiled executable.
use linkrange::*;
use std::{fs, process::Command};

struct Fixture {
    temp: tempfile::TempDir,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("source")).unwrap();
        Self { temp }
    }
    fn root(&self) -> std::path::PathBuf {
        self.temp.path().join("source")
    }
    fn write(&self, path: &str, content: impl AsRef<[u8]>) {
        let path = self.root().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
    fn request(&self, starts: &[&str], outlinks: u32, inlinks: u32) -> Request {
        Request {
            source_root: self.root(),
            index: IndexOptions {
                cache_directory: Some(self.temp.path().join("cache")),
                ..Default::default()
            },
            query: Query {
                starts: starts
                    .iter()
                    .map(|path| Start {
                        path: (*path).into(),
                        depths: None,
                    })
                    .collect(),
                depths: Depths { outlinks, inlinks },
                ..Default::default()
            },
        }
    }
}
fn paths(response: &Response) -> Vec<&str> {
    response
        .nodes
        .iter()
        .map(|node| node.file.path.as_str())
        .collect()
}
fn node<'a>(response: &'a Response, path: &str) -> &'a Node {
    response.nodes.iter().find(|n| n.file.path == path).unwrap()
}
fn field(key: &str) -> FrontmatterField {
    FrontmatterField {
        key: key.into(),
        substring: None,
    }
}

#[test]
fn every_hop_consumes_both_budgets_for_files_folders_and_mixed_starts() {
    // Given one outbound hop uses the last incoming allowance.
    let f = Fixture::new();
    f.write("folder/A.md", "[[B]]");
    f.write("B.md", "");
    f.write("Incoming.md", "[[B]]");
    for starts in [
        &["folder/A.md"][..],
        &["folder"][..],
        &["folder", "folder/A.md"][..],
    ] {
        // When the same graph is requested through each supported start form.
        let response = query(&f.request(starts, 3, 1)).unwrap();
        // Then incoming traversal is not revived at B.
        assert_eq!(paths(&response), ["B.md", "folder/A.md"]);
        assert_eq!(node(&response, "B.md").remaining_inlinks, 0);
        assert_eq!(node(&response, "B.md").remaining_outlinks, 2);
    }
    let f = Fixture::new();
    f.write("A.md", "");
    f.write("B.md", "[[A]]");
    f.write("C.md", "[[B]]");
    assert_eq!(
        paths(&query(&f.request(&["A.md"], 1, 3)).unwrap()),
        ["A.md", "B.md"],
        "Incoming hops also consume outgoing depth"
    );
}

#[test]
fn frontier_ignores_both_expansive_and_restrictive_overrides_and_remains_capped() {
    // Given A -> B -> C -> D -> E and an incoming link to C.
    let f = Fixture::new();
    for (path, text) in [
        ("A", "[[B]]"),
        ("B", "[[C]]"),
        ("C", "[[D]]"),
        ("D", "[[E]]"),
        ("E", ""),
        ("Incoming", "[[C]]"),
    ] {
        f.write(&format!("{path}.md"), text);
    }
    for override_depth in [0, 100] {
        let mut request = f.request(&["A.md"], 1, 0);
        request.query.frontier_depth = 2;
        request.query.rules.push(Rule {
            path: "C.md".into(),
            outlinks: Some(override_depth),
            inlinks: Some(100),
            ..Default::default()
        });
        // When two frontier hops encounter the override.
        let response = query(&request).unwrap();
        // Then C and D are frontier, E is beyond the cap, and incoming stays exhausted.
        assert_eq!(paths(&response), ["A.md", "B.md", "C.md", "D.md"]);
        assert_eq!(node(&response, "C.md").inclusion, Inclusion::Frontier);
        assert_eq!(node(&response, "D.md").remaining_outlinks, -2);
        assert_eq!(node(&response, "C.md").remaining_inlinks, 0);
        request.query.frontier_depth = 0;
        assert_eq!(paths(&query(&request).unwrap()), ["A.md", "B.md"]);
    }
}

#[test]
fn stop_includes_a_node_exclude_omits_it_and_independent_routes_remain_valid() {
    let f = Fixture::new();
    f.write("A.md", "[[B]] [[D]]");
    f.write("B.md", "[[C]]");
    f.write("C.md", "");
    f.write("D.md", "[[C]]");
    let mut request = f.request(&["A.md"], 3, 0);
    request.query.rules.push(Rule {
        path: "B.md".into(),
        stop: true,
        ..Default::default()
    });
    assert_eq!(
        paths(&query(&request).unwrap()),
        ["A.md", "B.md", "C.md", "D.md"]
    );
    request.query.rules[0].exclude = true;
    let response = query(&request).unwrap();
    assert_eq!(paths(&response), ["A.md", "C.md", "D.md"]);
    assert_eq!(node(&response, "C.md").route, ["A.md", "D.md", "C.md"]);
    f.write("A.md", "[[B]]");
    request.query.rules[0].exclude = false;
    assert_eq!(paths(&query(&request).unwrap()), ["A.md", "B.md"]);
}

#[test]
fn boundary_embeds_are_terminal_and_only_selected_types_get_the_exception() {
    let f = Fixture::new();
    f.write("A.md", "![[image.svg]] [[linked.svg]] ![[page.md]]");
    f.write("image.svg", "<svg><a href='Beyond.md'/></svg>");
    f.write("linked.svg", "");
    f.write("page.md", "[[Beyond]]");
    f.write("Beyond.md", "");
    let mut request = f.request(&["A.md"], 0, 0);
    request.query.boundary_embed_types = vec!["svg".into()];
    let response = query(&request).unwrap();
    assert_eq!(paths(&response), ["A.md", "image.svg"]);
    assert_eq!(
        node(&response, "image.svg").inclusion,
        Inclusion::EmbeddedAsset
    );
    request.query.rules.push(Rule {
        path: "image.svg".into(),
        outlinks: Some(100),
        ..Default::default()
    });
    assert_eq!(paths(&query(&request).unwrap()), ["A.md", "image.svg"]);
}

#[test]
fn html_boundary_embeds_are_included_without_following_hyperlinks_or_recursive_embeds() {
    let f = Fixture::new();
    // Given local assets, an embedded document with its own links, and ordinary hyperlinks.
    f.write(
        "page.html",
        r#"
        <link rel="stylesheet" href="style.css">
        <script src="script.js"></script>
        <img src="image.webp">
        <iframe src="embedded.html"></iframe>
        <object data="document.pdf"></object>
        <a href="linked.html">ordinary page</a><a href="linked.webp">ordinary image</a>
    "#,
    );
    f.write(
        "embedded.html",
        "<img src='nested.png'><a href='Beyond.md'>Beyond</a>",
    );
    for path in [
        "style.css",
        "script.js",
        "image.webp",
        "document.pdf",
        "linked.html",
        "linked.webp",
        "nested.png",
        "Beyond.md",
    ] {
        f.write(path, "");
    }
    let mut request = f.request(&["page.html"], 0, 0);
    assert_eq!(paths(&query(&request).unwrap()), ["page.html"]);
    // When HTML sources are opted into the direct-embed exception.
    request.query.boundary_embed_source_types = vec!["html".into()];
    request.query.rules.push(Rule {
        path: "embedded.html".into(),
        outlinks: Some(100),
        ..Default::default()
    });
    let response = query(&request).unwrap();
    // Then every direct embed crosses the boundary, but neither recursion nor overrides expand it.
    assert_eq!(
        paths(&response),
        [
            "document.pdf",
            "embedded.html",
            "image.webp",
            "page.html",
            "script.js",
            "style.css"
        ]
    );
    for path in [
        "document.pdf",
        "embedded.html",
        "image.webp",
        "script.js",
        "style.css",
    ] {
        assert_eq!(node(&response, path).inclusion, Inclusion::EmbeddedAsset);
        assert_eq!(node(&response, path).remaining_outlinks, -1);
    }
    // The CLI exposes the same opt-in rule.
    let output = Command::new(env!("CARGO_BIN_EXE_linkrange"))
        .args(["query", "--source-root"])
        .arg(f.root())
        .args([
            "--start",
            "page.html",
            "--outlinks",
            "0",
            "--boundary-embed-source-type",
            "html",
            "--no-cache",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let cli: Response = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(paths(&cli), paths(&response));
    // Exclusions and stopping at the source still take priority over the exception.
    request.query.rules.push(Rule {
        path: "document.pdf".into(),
        exclude: true,
        ..Default::default()
    });
    assert!(!paths(&query(&request).unwrap()).contains(&"document.pdf"));
    request.query.rules.push(Rule {
        path: "page.html".into(),
        stop: true,
        ..Default::default()
    });
    assert_eq!(paths(&query(&request).unwrap()), ["page.html"]);
    // Source-format opt-in doesn't broaden the caller's Markdown embedding policy.
    f.write("note.md", "![[embedded.html]] ![[image.webp]]");
    request.query.starts[0].path = "note.md".into();
    assert_eq!(paths(&query(&request).unwrap()), ["note.md"]);
}

#[test]
fn query_scoped_resolution_and_adjacency_include_outside_neighbors_without_admitting_them() {
    let f = Fixture::new();
    f.write("A.md", "[[B]] [[Missing]]");
    f.write("B.md", "");
    f.write("Outside.md", "[[A]]");
    f.write("Unrelated.md", "");
    let mut request = f.request(&["A.md"], 0, 0);
    request.query.adjacency = true;
    let response = query(&request).unwrap();
    assert_eq!(paths(&response), ["A.md"]);
    assert_eq!(response.adjacency["A.md"].inlinks, ["Outside.md"]);
    assert_eq!(response.adjacency["A.md"].outlinks, ["B.md"]);
    assert_eq!(response.links_by_source.len(), 1);
    assert_eq!(
        response.links_by_source["A.md"][0].target.as_deref(),
        Some("B.md")
    );
    assert_eq!(response.links_by_source["A.md"][1].target, None);
    assert!(response.edges.is_empty());
}

#[test]
fn wikilink_resolution_preserves_root_local_shallowest_and_lexical_precedence() {
    let f = Fixture::new();
    f.write("nested/A.md", "[[Idea|display]]");
    for name in [
        "Idea.md",
        "nested/Idea.md",
        "aaa/Idea.md",
        "zzz/Idea.md",
        "deep/deeper/Idea.md",
    ] {
        f.write(name, "");
    }
    let mut request = f.request(&["nested/A.md"], 1, 0);
    request.query.explain_resolution = true;
    for (remove, expected, reason) in [
        (None, "Idea.md", "sourceRoot"),
        (Some("Idea.md"), "nested/Idea.md", "sourceDirectory"),
        (
            Some("nested/Idea.md"),
            "aaa/Idea.md",
            "shallowestThenLexical",
        ),
        (Some("aaa/Idea.md"), "zzz/Idea.md", "shallowestThenLexical"),
    ] {
        if let Some(path) = remove {
            fs::remove_file(f.root().join(path)).unwrap();
        }
        let response = query(&request).unwrap();
        let link = &response.links_by_source["nested/A.md"][0];
        assert_eq!(link.target.as_deref(), Some(expected));
        assert_eq!(link.link_parsed_alias.as_deref(), Some("display"));
        assert_eq!(link.resolution.as_ref().unwrap().reason, reason);
    }
}

#[test]
fn html_svg_and_html_inside_markdown_classify_elements_and_ignore_comments_and_code() {
    let f = Fixture::new();
    f.write("A.md", "<img src='image.svg'>\n\n```html\n<img src='missing.png'>\n```\n\n<!-- <img src='missing.png'> -->\n\n[Page](page.html)");
    f.write(
        "image.svg",
        "<svg><image href='texture.png'/><a href='B.md'/></svg>",
    );
    f.write("B.md", "");
    f.write("texture.png", []);
    f.write("page.html","<!-- <a href='missing.md'> --> <a href='B.md'>B</a><script>const text = `<img src='missing.png'>`;</script><img src='texture.png'>");
    let response = query(&f.request(&["A.md"], 3, 0)).unwrap();
    assert_eq!(
        paths(&response),
        ["A.md", "B.md", "image.svg", "page.html", "texture.png"]
    );
    assert_eq!(response.links_by_source["A.md"].len(), 2);
    assert_eq!(response.links_by_source["page.html"].len(), 2);
    assert!(
        response.links_by_source["image.svg"]
            .iter()
            .find(|link| link.target.as_deref() == Some("texture.png"))
            .unwrap()
            .is_embedded
    );
    assert!(
        !response.links_by_source["image.svg"]
            .iter()
            .find(|link| link.target.as_deref() == Some("B.md"))
            .unwrap()
            .is_embedded
    );
}

#[test]
fn encoded_html_asset_urls_resolve_with_or_without_dot_slash_and_survive_cache_reuse() {
    let f = Fixture::new();
    for prefix in ["", "./"] {
        // Given the same stylesheet and deferred scripts with URL-encoded spaces.
        f.write(
            "site/page.html",
            format!(
                r#"
            <link rel="stylesheet" href="{prefix}change%20matrix.css">
            <script defer src="{prefix}change%20matrix.data.js"></script>
            <script defer src="{prefix}change%20matrix.js"></script>
        "#
            ),
        );
        let assets = [
            "change matrix.css",
            "change matrix.data.js",
            "change matrix.js",
        ];
        for asset in assets {
            f.write(&format!("site/{asset}"), "");
        }
        let mut request = f.request(&["site/page.html"], 1, 0);
        request.query.explain_resolution = true;
        // When queried from a freshly scanned index and then the persisted cache.
        for _ in 0..2 {
            let result = query(&request).unwrap();
            // Then all assets are included, retaining the original URL for rewriting.
            assert_eq!(
                paths(&result),
                [
                    "site/change matrix.css",
                    "site/change matrix.data.js",
                    "site/change matrix.js",
                    "site/page.html"
                ]
            );
            let links = &result.links_by_source["site/page.html"];
            assert_eq!(links.len(), 3);
            for asset in assets {
                let raw = format!("{prefix}{}", asset.replace(' ', "%20"));
                let link = links
                    .iter()
                    .find(|link| link.link_original_text == raw)
                    .unwrap();
                let target = format!("site/{asset}");
                assert_eq!(link.target.as_deref(), Some(target.as_str()));
                assert!(link.is_embedded);
                assert_eq!(link.resolution.as_ref().unwrap().reason, "relativePath");
            }
        }
        // The CLI exposes the same resolved graph.
        let output = Command::new(env!("CARGO_BIN_EXE_linkrange"))
            .args(["query", "--source-root"])
            .arg(f.root())
            .args(["--start", "site/page.html", "--no-cache"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let result: Response = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(paths(&result).len(), 4);
    }
}

#[test]
fn url_paths_decode_once_after_separating_query_and_fragment_in_all_markup_formats() {
    let f = Fixture::new();
    let href = "../assets%20dir/caf%C3%A9%23%3F%2520+file.svg?v=1#section";
    let target = "assets dir/café#?%20+file.svg";
    f.write(target, "");
    for (source, content) in [
        ("pages/A.md", format!("[asset]({href})")),
        ("pages/B.md", format!("<img src='{href}'>")),
        ("pages/C.html", format!("<img src='{href}'>")),
        ("pages/D.svg", format!("<svg><image href='{href}'/></svg>")),
    ] {
        f.write(source, content);
        let result = query(&f.request(&[source], 1, 0)).unwrap();
        let link = &result.links_by_source[source][0];
        assert_eq!(link.target.as_deref(), Some(target), "{source}");
        assert_eq!(link.link_original_text, href);
        assert_eq!(link.link_parsed_anchor.as_deref(), Some("section"));
        assert!(paths(&result).contains(&target));
    }
}

#[test]
fn standard_markdown_reference_links_are_resolved() {
    let f = Fixture::new();
    f.write("A.md", "[Page][id]\n\n[id]: B.md\n");
    f.write("B.md", "");
    assert_eq!(
        paths(&query(&f.request(&["A.md"], 1, 0)).unwrap()),
        ["A.md", "B.md"]
    );
}

#[test]
fn excalidraw_keeps_physical_path_and_reports_detected_format() {
    let f = Fixture::new();
    f.write("A.md", "![[Drawing]]");
    f.write(
        "Drawing.excalidraw.md",
        "---\nexcalidraw-plugin: parsed\n---\n[[B]]",
    );
    f.write("B.md", "");
    let response = query(&f.request(&["A.md"], 2, 0)).unwrap();
    assert_eq!(paths(&response), ["A.md", "B.md", "Drawing.excalidraw.md"]);
    assert_eq!(
        node(&response, "Drawing.excalidraw.md").file.format,
        "excalidraw"
    );
    assert_eq!(
        response.links_by_source["Drawing.excalidraw.md"][0].link_source_page_path,
        "Drawing.excalidraw.md"
    );
}

#[test]
fn frontmatter_parsing_requires_both_leading_delimiter_and_requested_substring() {
    let f = Fixture::new();
    f.write("A.md", "tracked: true\n---\n");
    f.write("B.md", "---\nother: [ malformed\n---\ntracked: true");
    f.write("C.md", "---\ntracked: true\nother: value\n---\n");
    let mut request = f.request(&[""], 0, 0);
    request.index.frontmatter = vec![field("tracked")];
    let response = query(&request).unwrap();
    assert_eq!(response.metrics.yaml_parses, 1);
    assert!(response.diagnostics.is_empty());
    assert_eq!(node(&response, "C.md").file.metadata["tracked"], true);
    assert!(node(&response, "A.md").file.metadata.is_empty());
    f.write("B.md", "---\ntracked: [ malformed\n---\n");
    let response = query(&request).unwrap();
    assert!(response.complete);
    assert_eq!(response.diagnostics.len(), 1);
    assert_eq!(response.diagnostics[0].code, "malformedFrontmatter");
    request.index.frontmatter.clear();
    let response = query(&request).unwrap();
    assert_eq!(response.metrics.yaml_parses, 0);
    assert!(response.diagnostics.is_empty());
}

#[test]
fn missing_frontmatter_delimiter_is_diagnosed_only_for_requested_substrings() {
    let f = Fixture::new();
    f.write("A.md", "---\nother: [broken");
    let mut request = f.request(&["A.md"], 0, 0);
    request.index.frontmatter = vec![field("tracked")];
    assert!(query(&request).unwrap().diagnostics.is_empty());
    f.write("A.md", "---\ntracked: true");
    assert_eq!(
        query(&request).unwrap().diagnostics[0].code,
        "malformedFrontmatter"
    );
}

#[test]
fn field_selection_changes_rescan_metadata_without_reparsing_unchanged_links() {
    let f = Fixture::new();
    f.write("A.md", "---\none: 1\ntwo: 2\n---\n[[B]]");
    f.write("B.md", "");
    let mut request = f.request(&["A.md"], 1, 0);
    request.index.frontmatter = vec![field("one")];
    assert_eq!(query(&request).unwrap().metrics.link_parses, 2);
    let warm = query(&request).unwrap();
    assert_eq!(warm.metrics.files_read, 0);
    assert_eq!(warm.metrics.yaml_parses, 0);
    request.index.frontmatter = vec![field("two")];
    let changed = query(&request).unwrap();
    assert_eq!(changed.metrics.link_parses, 0);
    assert_eq!(changed.metrics.files_read, 2);
    assert_eq!(node(&changed, "A.md").file.metadata.len(), 1);
    assert_eq!(node(&changed, "A.md").file.metadata["two"], 2);
}

#[test]
fn cache_rebuild_and_no_cache_match_incremental_results_after_edits_additions_and_deletions() {
    let f = Fixture::new();
    f.write("A.md", "[[Idea]]");
    f.write("nested/Idea.md", "");
    let mut request = f.request(&["A.md"], 2, 1);
    query(&request).unwrap();
    f.write("Idea.md", "[[A]]");
    f.write("Incoming.md", "[[A]]");
    let incremental = query(&request).unwrap();
    assert_eq!(incremental.metrics.files_read, 2);
    request.index.rebuild = true;
    let rebuilt = query(&request).unwrap();
    assert_eq!(paths(&incremental), paths(&rebuilt));
    assert_eq!(
        serde_json::to_value(&incremental.links_by_source).unwrap(),
        serde_json::to_value(&rebuilt.links_by_source).unwrap()
    );
    fs::remove_file(f.root().join("Idea.md")).unwrap();
    request.index.rebuild = false;
    let removed = query(&request).unwrap();
    assert_eq!(removed.metrics.files_read, 0);
    request.index.no_cache = true;
    request.index.cache_directory = Some(f.temp.path().join("unused"));
    assert_eq!(paths(&removed), paths(&query(&request).unwrap()));
    assert!(!f.temp.path().join("unused").exists());
}

#[test]
fn strict_index_failure_preserves_cache_and_best_effort_is_explicitly_incomplete() {
    let f = Fixture::new();
    f.write("A.md", "");
    let mut request = f.request(&["A.md"], 0, 0);
    query(&request).unwrap();
    let cache = fs::read_dir(f.temp.path().join("cache"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|ext| ext == "json"))
        .unwrap();
    let before = fs::read(&cache).unwrap();
    f.write("Broken.md", [0xff, 0xfe]);
    assert!(query(&request).is_err());
    assert_eq!(before, fs::read(&cache).unwrap());
    request.index.best_effort = true;
    let partial = query(&request).unwrap();
    assert!(!partial.complete);
    assert_eq!(paths(&partial), ["A.md"]);
    assert_eq!(partial.diagnostics[0].path, "Broken.md");
    assert_eq!(before, fs::read(&cache).unwrap());
}

#[cfg(unix)]
#[test]
fn symlinks_are_skipped_by_default_and_opt_in_stays_inside_root_deduplicates_and_detects_cycles() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    f.write("A.md", "[[alias]] [[escape]]");
    f.write("inside/real.md", "");
    fs::write(f.temp.path().join("outside.md"), "[[A]]").unwrap();
    symlink("inside/real.md", f.root().join("alias.md")).unwrap();
    symlink("inside/real.md", f.root().join("alias2.md")).unwrap();
    symlink(f.temp.path().join("outside.md"), f.root().join("escape.md")).unwrap();
    symlink("escape.md", f.root().join("chain.md")).unwrap();
    symlink("..", f.root().join("inside/loop")).unwrap();
    let mut request = f.request(&["A.md"], 1, 0);
    let skipped = query(&request).unwrap();
    assert_eq!(paths(&skipped), ["A.md"]);
    assert_eq!(
        skipped
            .diagnostics
            .iter()
            .filter(|d| d.code == "symlinkSkipped")
            .count(),
        5
    );
    request.index.symlinks = Symlinks::FollowInternal;
    let followed = query(&request).unwrap();
    assert_eq!(paths(&followed), ["A.md", "inside/real.md"]);
    assert!(followed.complete);
    assert_eq!(
        followed
            .diagnostics
            .iter()
            .filter(|d| d.code == "symlinkOutsideRoot")
            .count(),
        2
    );
    assert!(followed
        .diagnostics
        .iter()
        .any(|d| d.code == "symlinkCycle"));
    request.query.rules.push(Rule {
        path: "inside".into(),
        subtree: true,
        exclude: true,
        ..Default::default()
    });
    assert_eq!(paths(&query(&request).unwrap()), ["A.md"]);
}

#[test]
fn cli_exercises_public_request_and_emits_json_and_structured_errors() {
    let f = Fixture::new();
    f.write("A.md", "[[B]]");
    f.write("B.md", "");
    let path = f.temp.path().join("request.json");
    fs::write(
        &path,
        serde_json::to_vec(&f.request(&["A.md"], 1, 0)).unwrap(),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_linkrange"))
        .arg("query")
        .arg("--request")
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response: Response = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(paths(&response), ["A.md", "B.md"]);
    let output = Command::new(env!("CARGO_BIN_EXE_linkrange"))
        .args(["query", "--source-root"])
        .arg(f.root())
        .args(["--start", "Missing.md"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stderr).unwrap()["code"],
        "queryFailed"
    );
}

#[test]
fn relative_links_cannot_escape_root_and_accidentally_resolve_to_a_root_file() {
    let f = Fixture::new();
    f.write("A.md", "[escaped](../B.md)");
    f.write("B.md", "");
    let response = query(&f.request(&["A.md"], 1, 0)).unwrap();
    assert_eq!(paths(&response), ["A.md"]);
    assert!(response.links_by_source["A.md"][0].target.is_none());
}

#[test]
fn equally_short_routes_preserve_discovery_order_for_stable_breadcrumbs() {
    // Given two equal routes where the authored first branch sorts last.
    let fixture = Fixture::new();
    fixture.write("A.md", "[[Z]] [[B]]");
    fixture.write("Z.md", "[[Target]]");
    fixture.write("B.md", "[[Target]]");
    fixture.write("Target.md", "");
    // When the bounded graph is built, then the first discovered route wins.
    let response = query(&fixture.request(&["A.md"], 2, 0)).unwrap();
    assert_eq!(
        node(&response, "Target.md").route,
        vec!["A.md", "Z.md", "Target.md"]
    );
}

#[test]
fn explicit_physical_excalidraw_filenames_resolve_in_wikilinks_and_markdown() {
    let fixture = Fixture::new();
    fixture.write(
        "A.md",
        "[[nested/drawing.excalidraw.md|drawing]] [drawing](nested/drawing.excalidraw.md)",
    );
    fixture.write(
        "nested/drawing.excalidraw.md",
        "---\nexcalidraw-plugin: parsed\n---\n",
    );
    let mut request = fixture.request(&["A.md"], 1, 0);
    request.query.explain_resolution = true;
    let response = query(&request).unwrap();
    assert_eq!(
        paths(&response),
        vec!["A.md", "nested/drawing.excalidraw.md"]
    );
    assert_eq!(
        node(&response, "nested/drawing.excalidraw.md").file.format,
        "excalidraw"
    );
    for link in &response.links_by_source["A.md"] {
        assert_eq!(link.target.as_deref(), Some("nested/drawing.excalidraw.md"));
        assert_eq!(
            link.resolution.as_ref().unwrap().candidates,
            vec!["nested/drawing.excalidraw.md"]
        );
    }
}

#[test]
fn route_steps_preserve_the_arrival_that_enabled_each_hop_despite_a_shorter_display_route() {
    // Given Hub has a short arrival with no incoming budget, and a longer one
    // through an override that permits Incoming -> Hub to be followed backwards.
    let f = Fixture::new();
    f.write("Start.md", "[[Taxonomy]] [[Hub]]");
    f.write("Taxonomy.md", "[[Hub]]");
    f.write("Hub.md", "");
    f.write("Incoming.md", "[[Hub]] [[Target]]");
    f.write("Target.md", "");
    let mut request = f.request(&["Start.md"], 3, 1);
    request.query.rules.push(Rule {
        path: "Taxonomy.md".into(),
        outlinks: Some(3),
        inlinks: Some(2),
        ..Default::default()
    });
    // When queried cold, warm, and through the CLI, the same route evidence survives.
    let mut responses = vec![query(&request).unwrap(), query(&request).unwrap()];
    let request_file = f.temp.path().join("request.json");
    fs::write(&request_file, serde_json::to_vec(&request).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_linkrange"))
        .args(["query", "--request"])
        .arg(&request_file)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    responses.push(serde_json::from_slice(&output.stdout).unwrap());
    for response in responses {
        // Then Hub's own shortest explanation remains direct, with no incoming budget.
        let hub = node(&response, "Hub.md");
        assert_eq!(hub.route, ["Start.md", "Hub.md"]);
        assert_eq!((hub.depth, hub.remaining_inlinks), (1, 0));
        assert_eq!(
            hub.route_steps.last().unwrap().retained_for_traversal,
            Some(false)
        );
        assert_eq!(hub.alternative_routes.len(), 1);
        assert_eq!(
            hub.alternative_routes[0]
                .last()
                .unwrap()
                .retained_for_traversal,
            Some(true)
        );
        assert_eq!(
            hub.alternative_routes[0]
                .iter()
                .map(|step| step.path.as_str())
                .collect::<Vec<_>>(),
            ["Start.md", "Taxonomy.md", "Hub.md"]
        );
        assert_eq!(
            hub.alternative_routes[0].last().unwrap().remaining_inlinks,
            1
        );
        let target = node(&response, "Target.md");
        assert_eq!(
            target.route,
            [
                "Start.md",
                "Taxonomy.md",
                "Hub.md",
                "Incoming.md",
                "Target.md"
            ]
        );
        let steps = &target.route_steps;
        assert_eq!(
            steps
                .iter()
                .map(|step| (
                    step.depth,
                    step.remaining_outlinks,
                    step.remaining_inlinks,
                    step.via.as_str()
                ))
                .collect::<Vec<_>>(),
            [
                (0, 3, 1, "start"),
                (1, 3, 2, "outlink"),
                (2, 2, 1, "outlink"),
                (3, 1, 0, "inlink"),
                (4, 0, 0, "outlink")
            ]
        );
        assert_eq!(
            steps[1].inherited,
            Some(TraversalState {
                remaining_outlinks: 2,
                remaining_inlinks: 0
            })
        );
        assert_eq!(
            (steps[1].overridden_outlinks, steps[1].overridden_inlinks),
            (Some(3), Some(2))
        );
        for n in &response.nodes {
            assert_eq!(
                n.route_steps
                    .iter()
                    .map(|step| step.path.clone())
                    .collect::<Vec<_>>(),
                n.route
            );
            let last = n.route_steps.last().unwrap();
            assert_eq!(
                (
                    last.depth,
                    last.remaining_outlinks,
                    last.remaining_inlinks,
                    last.inclusion
                ),
                (
                    n.depth,
                    n.remaining_outlinks,
                    n.remaining_inlinks,
                    n.inclusion
                )
            );
        }
    }
}

#[test]
fn a_zero_override_preserves_the_stronger_arrival_for_explanation_without_traversing_it() {
    // Given a direct 2/0 arrival and a longer 2/1 arrival, both reset to 2/0 at Hub.
    let f = Fixture::new();
    f.write("Taxonomy.md", "[[Hub]]");
    f.write("Hub.md", "[[Tail]]");
    f.write("Tail.md", "");
    f.write("Incoming.md", "[[Hub]] [[Target]]");
    f.write("Target.md", "");
    let mut request = f.request(&["Start.md"], 3, 1);
    request.query.rules = vec![
        Rule {
            path: "Taxonomy.md".into(),
            outlinks: Some(3),
            inlinks: Some(2),
            ..Default::default()
        },
        Rule {
            path: "Hub.md".into(),
            inlinks: Some(0),
            ..Default::default()
        },
    ];
    for links in ["[[Taxonomy]] [[Hub]]", "[[Hub]] [[Taxonomy]]"] {
        f.write("Start.md", links);
        // When the override collapses the arrivals to one useful traversal state.
        let response = query(&request).unwrap();
        let hub = node(&response, "Hub.md");
        assert_eq!(hub.route, ["Start.md", "Hub.md"]);
        assert_eq!(
            hub.states,
            [TraversalState {
                remaining_outlinks: 2,
                remaining_inlinks: 0
            }]
        );
        // Then the longer route still explains the actual 1 -> 0 override.
        assert_eq!(hub.alternative_routes.len(), 1);
        let steps = &hub.alternative_routes[0];
        assert_eq!(
            steps.iter().map(|s| s.path.as_str()).collect::<Vec<_>>(),
            ["Start.md", "Taxonomy.md", "Hub.md"]
        );
        let arrival = steps.last().unwrap();
        assert_eq!(
            hub.route_steps.last().unwrap().retained_for_traversal,
            Some(true)
        );
        assert_eq!(arrival.retained_for_traversal, Some(false));
        assert_eq!(arrival.inherited.as_ref().unwrap().remaining_inlinks, 1);
        assert_eq!(arrival.overridden_inlinks, Some(0));
        assert_eq!(arrival.remaining_inlinks, 0);
        // Explanation-only arrivals never revive incoming traversal or propagate duplicate routes.
        assert!(!paths(&response).contains(&"Incoming.md"));
        assert!(!paths(&response).contains(&"Target.md"));
        assert!(node(&response, "Tail.md").alternative_routes.is_empty());
    }
}

#[test]
fn overrides_preserve_separate_pre_override_maxima_without_retaining_every_redundant_arrival() {
    // Given three different inherited budget pairs that two overrides collapse to 2/0.
    let f = Fixture::new();
    for start in ["Out", "Balanced", "In"] {
        f.write(&format!("{start}.md"), "[[Hub]]");
    }
    f.write("Hub.md", "[[Out]]");
    let mut request = f.request(&["Out.md", "Balanced.md", "In.md"], 0, 0);
    for (start, (outlinks, inlinks)) in
        request
            .query
            .starts
            .iter_mut()
            .zip([(6, 1), (4, 3), (2, 5)])
    {
        start.depths = Some(Depths { outlinks, inlinks });
    }
    request.query.rules.push(Rule {
        path: "Hub.md".into(),
        outlinks: Some(2),
        inlinks: Some(0),
        ..Default::default()
    });
    // When querying a graph that also contains a cycle.
    let response = query(&request).unwrap();
    let hub = node(&response, "Hub.md");
    let routes: Vec<_> = std::iter::once(&hub.route_steps)
        .chain(&hub.alternative_routes)
        .collect();
    // Then each maximum has its real route, never a fabricated 5/4 inherited pair.
    assert_eq!(
        routes
            .iter()
            .map(|r| r.last().unwrap().inherited.clone().unwrap())
            .collect::<Vec<_>>(),
        [
            TraversalState {
                remaining_outlinks: 5,
                remaining_inlinks: 0
            },
            TraversalState {
                remaining_outlinks: 1,
                remaining_inlinks: 4
            },
        ]
    );
    assert_eq!(
        hub.states,
        [TraversalState {
            remaining_outlinks: 2,
            remaining_inlinks: 0
        }]
    );
}

#[test]
fn alternative_routes_preserve_every_useful_budget_pair_without_synthesizing_the_maxima() {
    // Given three starts supply Hub with 5/0, 3/2, and 1/4 remaining budgets.
    let f = Fixture::new();
    for start in ["Out", "Balanced", "In"] {
        f.write(&format!("{start}.md"), "[[Hub]]");
    }
    f.write("Hub.md", "");
    f.write("Incoming1.md", "[[Hub]]");
    f.write("Incoming2.md", "[[Incoming1]]");
    f.write("Incoming3.md", "[[Incoming2]]");
    let mut request = f.request(&["Out.md", "Balanced.md", "In.md"], 0, 0);
    for (start, (outlinks, inlinks)) in
        request
            .query
            .starts
            .iter_mut()
            .zip([(6, 1), (4, 3), (2, 5)])
    {
        start.depths = Some(Depths { outlinks, inlinks });
    }
    // When the graph is queried, each useful pair retains its own complete explanation.
    let response = query(&request).unwrap();
    let hub = node(&response, "Hub.md");
    let routes: Vec<_> = std::iter::once(&hub.route_steps)
        .chain(&hub.alternative_routes)
        .collect();
    assert_eq!(
        routes
            .iter()
            .map(|route| {
                let step = route.last().unwrap();
                (step.remaining_outlinks, step.remaining_inlinks)
            })
            .collect::<Vec<_>>(),
        [(5, 0), (3, 2), (1, 4)]
    );
    assert_eq!(
        routes
            .iter()
            .map(|route| route[0].path.as_str())
            .collect::<Vec<_>>(),
        ["Out.md", "Balanced.md", "In.md"]
    );
    // Then the intermediate tradeoff is retained, and the synthetic 5/4 arrival does not exist.
    assert!(routes
        .iter()
        .all(|route| route.last().unwrap().retained_for_traversal == Some(true)));
    assert!(paths(&response).contains(&"Incoming2.md"));
    assert!(!paths(&response).contains(&"Incoming3.md"));
    assert!(!hub
        .states
        .iter()
        .any(|state| state.remaining_outlinks == 5 && state.remaining_inlinks == 4));
}
