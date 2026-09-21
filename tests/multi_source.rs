use linkrange::*;
use std::{collections::BTreeMap, fs, process::Command};

struct Fixture(tempfile::TempDir);
impl Fixture {
    fn new() -> Self {
        Self(tempfile::tempdir().unwrap())
    }
    fn source(&self, name: &str, aliases: &[&str]) -> Source {
        let directory = self.0.path().join(name);
        fs::create_dir_all(&directory).unwrap();
        Source {
            name: name.into(),
            directory,
            aliases: aliases.iter().map(|s| (*s).into()).collect(),
        }
    }
    fn write(&self, source: &Source, path: &str, body: &str) {
        let path = source.directory.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }
    fn request(&self, sources: Vec<Source>, start: &str, outlinks: u32, inlinks: u32) -> Request {
        Request {
            sources,
            index: IndexOptions {
                cache_directory: Some(self.0.path().join("cache")),
                ..Default::default()
            },
            query: Query {
                starts: vec![Start {
                    path: start.into(),
                    depths: None,
                }],
                depths: Depths { outlinks, inlinks },
                adjacency: true,
                explain_resolution: true,
                metrics: true,
                ..Default::default()
            },
        }
    }
}
fn paths(response: &Response) -> Vec<&str> {
    response
        .nodes
        .iter()
        .map(|n| n.file.path.as_str())
        .collect()
}
fn node<'a>(response: &'a Response, path: &str) -> &'a Node {
    response.nodes.iter().find(|n| n.file.path == path).unwrap()
}

#[test]
fn multi_source_resolution_is_local_by_default_and_preserves_alias_provenance() {
    let f = Fixture::new();
    let notes = f.source("notes", &[]);
    let research = f.source("research", &["papers"]);
    f.write(&notes, "nested/Start.md", "[[Overview]] [[Overview::papers#Summary|Research overview]] [exact](source://research/Space%20%23%3F%25.md?q=1#part) ![[image.png::papers|300]] [[Only there]]");
    f.write(&notes, "Overview.md", "local");
    f.write(&research, "Overview.md", "research");
    f.write(
        &research,
        "nested/Overview.md",
        "must not borrow origin directory",
    );
    f.write(&research, "Only there.md", "no implicit fallback");
    f.write(&research, "Space #?%.md", "escaped");
    f.write(&research, "image.png", "image");
    let request = f.request(
        vec![notes, research],
        "source://notes/nested/Start.md",
        1,
        0,
    );
    let result = query(&request).unwrap();
    assert_eq!(result.schema_version, 2);
    assert_eq!(result.sources.len(), 2);
    assert_eq!(
        paths(&result),
        [
            "source://notes/Overview.md",
            "source://notes/nested/Start.md",
            "source://research/Overview.md",
            "source://research/Space%20%23%3F%25.md",
            "source://research/image.png"
        ]
    );
    let links = &result.links_by_source["source://notes/nested/Start.md"];
    assert_eq!(
        links[0].link_resolved_target_source.as_deref(),
        Some("notes")
    );
    assert_eq!(
        links[1].link_requested_target_source.as_deref(),
        Some("papers")
    );
    assert_eq!(
        links[1].link_resolved_target_source.as_deref(),
        Some("research")
    );
    assert_eq!(links[1].link_parsed_anchor.as_deref(), Some("Summary"));
    assert_eq!(
        links[1].link_parsed_alias.as_deref(),
        Some("Research overview")
    );
    assert_eq!(
        links[1].link_original_text,
        "Overview::papers#Summary|Research overview"
    );
    assert_eq!(
        links[1].resolution.as_ref().unwrap().candidates[0],
        "source://research/Overview.md"
    );
    assert_eq!(links[2].link_parsed_anchor.as_deref(), Some("part"));
    assert!(links[3].is_embedded);
    assert_eq!(links[3].link_parsed_media_size, Some(300));
    assert!(links[4].target.is_none());
    assert!(result
        .edges
        .iter()
        .all(|edge| edge.source.starts_with("source://") && edge.target.starts_with("source://")));
}

#[test]
fn qualified_wikilinks_require_a_page_target() {
    let f = Fixture::new();
    let notes = f.source("notes", &[]);
    f.write(
        &notes,
        "Start.md",
        "[[::notes]] [[::notes#Heading]] [[::notes|Alias]]",
    );
    let request = f.request(vec![notes], "source://notes/Start.md", 1, 0);
    let response = query(&request).unwrap();
    assert_eq!(paths(&response), ["source://notes/Start.md"]);
    assert_eq!(response.diagnostics.len(), 3);
    assert!(response
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == "invalidSourceReference"));
    assert!(response.links_by_source["source://notes/Start.md"]
        .iter()
        .all(|link| link.target.is_none()));
}

#[test]
fn unknown_sources_are_query_scoped_occurrences_with_one_registered_source() {
    let f = Fixture::new();
    let notes = f.source("notes", &[]);
    f.write(
        &notes,
        "Start.md",
        "[[Overview::research]] [url](source://research/Overview.md) [[Frontier]]",
    );
    f.write(&notes, "Overview.md", "local namesake must not resolve");
    f.write(&notes, "Frontier.md", "[[Other::archive]]");
    f.write(&notes, "Unrelated.md", "[[Hidden::unrelated]]");
    let mut request = f.request(vec![notes], "source://notes/Start.md", 0, 0);
    let normal = query(&request).unwrap();
    assert_eq!(normal.schema_version, SCHEMA_VERSION);
    assert_eq!(paths(&normal), ["source://notes/Start.md"]);
    assert_eq!(normal.diagnostics.len(), 2);
    assert!(normal.diagnostics.iter().all(
        |d| d.code == "unregisteredSource" && d.requested_source.as_deref() == Some("research")
    ));
    assert_eq!(normal.links_by_source["source://notes/Start.md"].len(), 3);
    assert!(normal.links_by_source["source://notes/Start.md"][0]
        .target
        .is_none());
    request.query.frontier_depth = 1;
    let frontier = query(&request).unwrap();
    assert_eq!(frontier.diagnostics.len(), 3);
    assert_eq!(
        node(&frontier, "source://notes/Frontier.md").inclusion,
        Inclusion::Frontier
    );
    request.query.depths.outlinks = 1;
    let traversed = query(&request).unwrap();
    assert_eq!(
        node(&traversed, "source://notes/Frontier.md").inclusion,
        Inclusion::Traversal
    );
    assert_eq!(traversed.diagnostics.len(), 3);
}

#[test]
fn incoming_cross_source_arrivals_keep_budgets_routes_cycles_and_overrides() {
    let f = Fixture::new();
    let notes = f.source("notes", &[]);
    let research = f.source("research", &["papers"]);
    f.write(&notes, "Start.md", "");
    f.write(&research, "Incoming.md", "[[Start::notes]] [[Tail]]");
    f.write(&research, "Tail.md", "[[Cycle::notes]]");
    f.write(&notes, "Cycle.md", "[[Incoming::research]]");
    let mut request = f.request(vec![notes, research], "source://notes/Start.md", 3, 1);
    let result = query(&request).unwrap();
    let tail = node(&result, "source://research/Tail.md");
    assert_eq!(
        tail.route,
        [
            "source://notes/Start.md",
            "source://research/Incoming.md",
            "source://research/Tail.md"
        ]
    );
    assert_eq!(
        (tail.depth, tail.remaining_outlinks, tail.remaining_inlinks),
        (2, 1, 0)
    );
    assert_eq!(tail.route_steps[1].via, "inlink");
    assert_eq!(
        node(&result, "source://notes/Cycle.md").remaining_outlinks,
        0
    );
    assert_eq!(
        result.adjacency["source://notes/Start.md"].inlinks,
        ["source://research/Incoming.md"]
    );
    request.query.rules.push(Rule {
        path: "source://papers/Incoming.md".into(),
        stop: true,
        ..Default::default()
    });
    assert_eq!(
        paths(&query(&request).unwrap()),
        ["source://notes/Start.md", "source://research/Incoming.md"]
    );
    request.query.rules[0].stop = false;
    request.query.rules[0].exclude = true;
    assert_eq!(
        paths(&query(&request).unwrap()),
        ["source://notes/Start.md"]
    );
    request.query.rules[0].exclude = false;
    request.query.rules[0].outlinks = Some(0);
    request.query.frontier_depth = 1;
    assert_eq!(
        node(&query(&request).unwrap(), "source://research/Tail.md").inclusion,
        Inclusion::Frontier
    );
}

#[test]
fn source_url_links_and_embeds_work_in_html_svg_and_markdown_without_escaping_roots() {
    let f = Fixture::new();
    let notes = f.source("notes", &[]);
    let media = f.source("media", &[]);
    f.write(&notes, "Start.html", "<a href='source://media/Target.md'>target</a><img src='source://media/image.png'><object data='source://media/Drawing.svg'></object><a href='source://media/%2e%2e/Secret.md'>escape</a><a href='source://media/Bad%GG.md'>invalid</a>");
    f.write(&media, "Target.md", "");
    f.write(&media, "image.png", "image");
    f.write(&media, "Drawing.svg", "<svg><a href='source://notes/Start.html'>back</a><image href='source://media/image.png'/></svg>");
    let mut request = f.request(vec![notes, media], "source://notes/Start.html", 0, 0);
    request.query.boundary_embed_source_types = vec!["html".into()];
    let result = query(&request).unwrap();
    assert_eq!(
        paths(&result),
        [
            "source://media/Drawing.svg",
            "source://media/image.png",
            "source://notes/Start.html"
        ]
    );
    assert_eq!(
        node(&result, "source://media/Drawing.svg").inclusion,
        Inclusion::EmbeddedAsset
    );
    assert_eq!(result.diagnostics.len(), 2);
    assert!(result
        .diagnostics
        .iter()
        .all(|d| d.code == "invalidSourceReference"));
    assert_eq!(
        result.links_by_source["source://media/Drawing.svg"][0]
            .target
            .as_deref(),
        Some("source://notes/Start.html")
    );
    assert!(result.links_by_source["source://media/Drawing.svg"][1].is_embedded);
}

#[test]
fn mixed_source_folder_and_file_starts_cost_no_membership_budget_and_rules_stay_local() {
    let f = Fixture::new();
    let notes = f.source("notes", &[]);
    let research = f.source("research", &[]);
    f.write(&notes, "Start.md", "[[Tail]]");
    f.write(&notes, "Tail.md", "");
    f.write(&research, "folder/Start.md", "[[Tail]]");
    f.write(&research, "folder/Tail.md", "");
    let mut request = f.request(vec![notes, research], "source://notes/Start.md", 1, 0);
    request.query.starts.push(Start {
        path: "source://research/".into(),
        depths: Some(Depths {
            outlinks: 0,
            inlinks: 0,
        }),
    });
    let result = query(&request).unwrap();
    assert_eq!(result.nodes.len(), 4);
    assert_eq!(node(&result, "source://notes/Tail.md").depth, 1);
    assert_eq!(node(&result, "source://research/folder/Tail.md").depth, 0);
    request.query.rules.push(Rule {
        path: "source://research/".into(),
        subtree: true,
        exclude: true,
        ..Default::default()
    });
    assert_eq!(
        paths(&query(&request).unwrap()),
        ["source://notes/Start.md", "source://notes/Tail.md"]
    );
}

#[test]
fn cache_reuse_is_independent_of_names_aliases_starts_and_selected_sources() {
    let f = Fixture::new();
    let notes = f.source("notes", &[]);
    let research = f.source("research", &[]);
    f.write(&notes, "Start.md", "[[Target::papers]]");
    let mut request = f.request(
        vec![notes, research.clone()],
        "source://notes/Start.md",
        1,
        0,
    );
    let cold = query(&request).unwrap();
    assert_eq!(cold.metrics.unwrap().link_parses, 1);
    request.sources[1].aliases.push("papers".into());
    let registered = query(&request).unwrap();
    assert_eq!(registered.metrics.unwrap().link_parses, 0);
    assert!(registered.diagnostics.is_empty());
    assert!(registered.links_by_source["source://notes/Start.md"][0]
        .target
        .is_none());
    f.write(&research, "Target.md", "[[Start::notes]]");
    let added = query(&request).unwrap();
    assert_eq!(added.metrics.unwrap().link_parses, 1);
    assert_eq!(added.nodes.len(), 2);
    request.sources[1].name = "library".into();
    request.sources[1].aliases.push("research".into());
    let renamed = query(&request).unwrap();
    assert_eq!(renamed.metrics.unwrap().link_parses, 0);
    assert_eq!(
        renamed.links_by_source["source://notes/Start.md"][0]
            .target
            .as_deref(),
        Some("source://library/Target.md")
    );
    request.sources.remove(1);
    let removed = query(&request).unwrap();
    assert_eq!(removed.metrics.unwrap().link_parses, 0);
    assert_eq!(removed.diagnostics.len(), 1);
}

#[test]
fn registry_validation_rejects_conflicts_overlap_and_unavailability_without_cache_damage() {
    let f = Fixture::new();
    let notes = f.source("notes", &["old"]);
    let research = f.source("research", &[]);
    f.write(&notes, "Start.md", "");
    f.write(&research, "Other.md", "");
    let request = f.request(
        vec![notes.clone(), research.clone()],
        "source://notes/Start.md",
        0,
        0,
    );
    query(&request).unwrap();
    let cached = || -> BTreeMap<_, _> {
        fs::read_dir(f.0.path().join("cache"))
            .unwrap()
            .map(|p| p.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .map(|p| (p.clone(), fs::read(p).unwrap()))
            .collect()
    };
    let before = cached();
    let check = |sources: Vec<Source>, message: &str| {
        let bad = Request {
            sources,
            ..request.clone()
        };
        assert!(format!("{:#}", query(&bad).unwrap_err()).contains(message));
        assert_eq!(cached(), before);
    };
    check(
        vec![
            notes.clone(),
            Source {
                name: "old".into(),
                ..research.clone()
            },
        ],
        "already registered",
    );
    check(
        vec![
            notes.clone(),
            Source {
                directory: notes.directory.clone(),
                ..research.clone()
            },
        ],
        "overlapping",
    );
    fs::create_dir_all(notes.directory.join("nested")).unwrap();
    check(
        vec![
            notes.clone(),
            Source {
                directory: notes.directory.join("nested"),
                ..research.clone()
            },
        ],
        "overlapping",
    );
    check(
        vec![
            notes.clone(),
            Source {
                name: "Bad Name".into(),
                ..research.clone()
            },
        ],
        "Invalid source name",
    );
    fs::rename(&research.directory, f.0.path().join("disconnected")).unwrap();
    check(vec![notes, research], "unavailable");
    check(vec![], "At least one source");
}

#[test]
fn cli_accepts_source_registry_and_rejects_conflicting_request_forms() {
    let f = Fixture::new();
    let notes = f.source("notes", &[]);
    f.write(&notes, "Start.md", "");
    let request = f.request(vec![notes.clone()], "source://notes/Start.md", 0, 0);
    let path = f.0.path().join("request.json");
    fs::write(&path, serde_json::to_vec(&request).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_linkrange"))
        .args(["query", "--request"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Response>(&output.stdout)
            .unwrap()
            .sources
            .len(),
        1
    );
    let mut invalid = serde_json::to_value(&request).unwrap();
    invalid["sourceRoot"] = serde_json::to_value(&notes.directory).unwrap();
    fs::write(&path, serde_json::to_vec(&invalid).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_linkrange"))
        .args(["query", "--request"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stderr).unwrap()["code"],
        "queryFailed"
    );
    assert!(!Command::new(env!("CARGO_BIN_EXE_linkrange"))
        .args(["query", "--request"])
        .arg(path)
        .arg("--source")
        .arg(format!("notes={}", notes.directory.display()))
        .output()
        .unwrap()
        .status
        .success());
}

#[test]
fn requests_require_sources_and_qualified_selectors_even_with_one_source() {
    let f = Fixture::new();
    let notes = f.source("notes", &[]);
    f.write(&notes, "Start.md", "");
    let request = f.request(vec![notes], "source://notes/Start.md", 0, 0);
    let mut json = serde_json::to_value(&request).unwrap();
    json.as_object_mut().unwrap().remove("sources");
    assert!(serde_json::from_value::<Request>(json.clone()).is_err());
    json["sourceRoot"] = f.0.path().to_string_lossy().into_owned().into();
    assert!(serde_json::from_value::<Request>(json).is_err());
    assert!(query(&Request {
        sources: vec![],
        ..request.clone()
    })
    .is_err());

    for selector in ["start", "rule", "lookup"] {
        let mut invalid = request.clone();
        match selector {
            "start" => invalid.query.starts[0].path = "Start.md".into(),
            "rule" => invalid.query.rules.push(Rule {
                path: "Start.md".into(),
                stop: true,
                ..Default::default()
            }),
            _ => invalid.query.lookup_paths.push("Start.md".into()),
        }
        assert!(
            query(&invalid)
                .unwrap_err()
                .to_string()
                .contains("Expected source://"),
            "{selector}"
        );
    }
}

#[test]
fn repeated_cli_sources_use_the_same_schema_as_json_for_one_or_many_sources() {
    let f = Fixture::new();
    let notes = f.source("notes", &[]);
    let research = f.source("research", &[]);
    f.write(&notes, "Start.md", "[[Overview::research]]");
    f.write(&research, "Overview.md", "");
    for sources in [vec![notes.clone()], vec![notes.clone(), research]] {
        let mut request = f.request(sources.clone(), "source://notes/Start.md", 1, 0);
        request.query.metrics = false;
        let expected = serde_json::to_value(query(&request).unwrap()).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_linkrange"));
        command.arg("query");
        for source in sources {
            command
                .arg("--source")
                .arg(format!("{}={}", source.name, source.directory.display()));
        }
        let output = command
            .args([
                "--start",
                "source://notes/Start.md",
                "--adjacency",
                "--explain-resolution",
                "--no-cache",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let actual: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(actual["schemaVersion"], SCHEMA_VERSION);
        assert_eq!(
            actual["sources"].as_array().unwrap().len(),
            request.sources.len()
        );
        assert!(serde_json::from_value::<Response>(actual.clone()).is_ok());
        let mut missing_registry = actual.clone();
        missing_registry.as_object_mut().unwrap().remove("sources");
        assert!(serde_json::from_value::<Response>(missing_registry).is_err());
        let mut missing_route_status = actual;
        missing_route_status["nodes"][0]["routeSteps"][0]
            .as_object_mut()
            .unwrap()
            .remove("retainedForTraversal");
        assert!(serde_json::from_value::<Response>(missing_route_status).is_err());
    }
    let removed_flag = Command::new(env!("CARGO_BIN_EXE_linkrange"))
        .args(["query", "--source-root"])
        .arg(&notes.directory)
        .args(["--start", "Start.md"])
        .output()
        .unwrap();
    assert!(!removed_flag.status.success());
}

#[cfg(unix)]
#[test]
fn a_symlink_cannot_implicitly_connect_admitted_sources() {
    let f = Fixture::new();
    let notes = f.source("notes", &[]);
    let research = f.source("research", &[]);
    f.write(&notes, "Start.md", "[[alias]]");
    f.write(&research, "Other.md", "");
    std::os::unix::fs::symlink(
        research.directory.join("Other.md"),
        notes.directory.join("alias.md"),
    )
    .unwrap();
    let mut request = f.request(vec![notes, research], "source://notes/Start.md", 1, 0);
    request.index.symlinks = Symlinks::FollowInternal;
    let result = query(&request).unwrap();
    assert_eq!(paths(&result), ["source://notes/Start.md"]);
    assert_eq!(result.diagnostics[0].code, "symlinkOutsideRoot");
    assert_eq!(result.diagnostics[0].path, "source://notes/alias.md");
}
