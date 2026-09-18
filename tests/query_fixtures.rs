//! Query results are checked against per-file expectations and committed output snapshots.
use linkrange::*;
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    name: String,
    source: String,
    query: Query,
    expectation_count: Option<usize>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Expectation {
    #[serde(skip)]
    path: String,
    is_in_working_graph: bool,
    frontier_depth_or_null_for_orphan: Option<i64>,
    links: Option<Links>,
}
#[derive(Deserialize)]
struct Links {
    #[serde(default)]
    inlinks: Vec<ExpectedLink>,
    #[serde(default)]
    outlinks: Vec<ExpectedLink>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExpectedLink {
    link_path: String,
    is_in_graph: bool,
}

fn files_under(directory: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            files.extend(files_under(&entry.path()));
        } else {
            files.push(entry.path());
        }
    }
    files.sort();
    files
}

fn read_json<T: DeserializeOwned>(path: &Path) -> T {
    let bytes = fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    serde_json::from_slice(&bytes).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn compare_output(expected_path: &Path, actual_path: &Path, actual: &Value) -> Result<(), String> {
    let problem = if expected_path.is_file() {
        let expected: Value = read_json(expected_path);
        if expected == *actual {
            return Ok(());
        }
        "Output differs from"
    } else {
        "Missing expected output"
    };
    fs::create_dir_all(actual_path.parent().unwrap()).unwrap();
    fs::write(
        actual_path,
        format!("{}\n", serde_json::to_string_pretty(actual).unwrap()),
    )
    .unwrap();
    Err(format!(
        "{problem} {}; actual output saved to {}",
        expected_path.display(),
        actual_path.display()
    ))
}

#[test]
fn output_comparison_detects_changes_without_rewriting_expected_outputs() {
    let temp = tempfile::tempdir().unwrap();
    let expected_path = temp.path().join("expected.json");
    let actual_path = temp.path().join("actual/output.json");
    let original = "{\"nodes\":[{\"path\":\"A.md\",\"depth\":0}]}\n";
    let mut actual: Value = serde_json::from_str(original).unwrap();
    fs::write(&expected_path, original).unwrap();
    assert!(compare_output(&expected_path, &actual_path, &actual).is_ok());
    assert!(!actual_path.exists());

    actual["nodes"][0]["depth"] = 1.into();
    assert!(compare_output(&expected_path, &actual_path, &actual).is_err());
    assert_eq!(fs::read_to_string(&expected_path).unwrap(), original);
    assert_eq!(read_json::<Value>(&actual_path), actual);

    fs::remove_file(&expected_path).unwrap();
    assert!(compare_output(&expected_path, &actual_path, &actual).is_err());
    assert!(!expected_path.exists());
}

#[test]
fn query_results_match_fixture_expectations() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let expected_outputs = fixture.join("expected_outputs");
    let actual_outputs = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/fixture_outputs");
    let files = files_under(&fixture);
    let query_files: Vec<_> = files
        .iter()
        .filter(|path| path.to_string_lossy().ends_with(".query.json"))
        .collect();
    assert!(!query_files.is_empty(), "No query fixtures were found");
    let cache = tempfile::tempdir().unwrap();
    let mut errors = Vec::new();
    let mut assertions = 0;
    let mut used_specs = BTreeSet::new();
    let mut used_outputs = BTreeSet::new();
    let mut names = BTreeSet::new();
    for query_file in query_files {
        let case: Case = read_json(query_file);
        assert!(
            names.insert(case.name.clone()),
            "Duplicate fixture: {}",
            case.name
        );
        assert_eq!(
            query_file.file_name().unwrap().to_str().unwrap(),
            format!("{}.query.json", case.name),
            "Query filename must match its fixture name"
        );
        let source = fixture.join(&case.source);
        let suffix = format!(".nodespec-{}.json", case.name);
        let mut expected = Vec::new();
        for spec in files
            .iter()
            .filter(|path| path.starts_with(&source) && path.to_string_lossy().ends_with(&suffix))
        {
            let relative = spec
                .strip_prefix(&source)
                .unwrap()
                .to_str()
                .unwrap()
                .replace('\\', "/");
            let path = relative.strip_suffix(&suffix).unwrap();
            assert!(
                source.join(path).is_file(),
                "{} has no source file",
                spec.display()
            );
            let mut expectation: Expectation = read_json(spec);
            expectation.path = path.into();
            expected.push(expectation);
            used_specs.insert(spec);
        }
        assert_eq!(
            expected.len(),
            case.expectation_count.unwrap_or(0),
            "{}: node expectation count changed",
            case.name
        );
        let graph = Graph::open(
            &source,
            &IndexOptions {
                cache_directory: Some(cache.path().into()),
                ..Default::default()
            },
        )
        .unwrap();
        let normal = graph.query(&case.query).unwrap();
        let output = serde_json::to_value(&normal).unwrap();
        let expected_output = expected_outputs.join(format!("{}.json", case.name));
        if let Err(error) = compare_output(
            &expected_output,
            &actual_outputs.join(format!("{}.json", case.name)),
            &output,
        ) {
            errors.push(format!("{}: {error}", case.name));
        }
        used_outputs.insert(expected_output);
        if case.expectation_count.is_none() {
            assert!(normal.complete, "{}", case.name);
            continue;
        }
        let actual: BTreeSet<_> = normal.nodes.iter().map(|n| n.file.path.clone()).collect();
        let wanted: BTreeSet<_> = expected
            .iter()
            .filter(|e| e.is_in_working_graph)
            .map(|e| e.path.clone())
            .collect();
        if actual != wanted {
            errors.push(format!(
                "{} membership: missing {:?}; unexpected {:?}",
                case.name,
                wanted.difference(&actual).collect::<Vec<_>>(),
                actual.difference(&wanted).collect::<Vec<_>>()
            ));
        }
        let mut extended_query = case.query.clone();
        extended_query.frontier_depth = 10;
        let extended = graph.query(&extended_query).unwrap();
        for expectation in expected {
            assertions += 1;
            if !expectation.is_in_working_graph {
                let frontier = extended
                    .nodes
                    .iter()
                    .find(|node| node.file.path == expectation.path)
                    .map(|node| -node.remaining_outlinks);
                if frontier != expectation.frontier_depth_or_null_for_orphan {
                    errors.push(format!(
                        "{} {} frontier: expected {:?}, actual {:?}",
                        case.name,
                        expectation.path,
                        expectation.frontier_depth_or_null_for_orphan,
                        frontier
                    ));
                }
            }
            if let Some(links) = expectation.links {
                for (direction, expected_links) in
                    [("inlinks", links.inlinks), ("outlinks", links.outlinks)]
                {
                    let actual_links: BTreeSet<String> = normal
                        .adjacency
                        .get(&expectation.path)
                        .map(|adj| {
                            if direction == "inlinks" {
                                &adj.inlinks
                            } else {
                                &adj.outlinks
                            }
                        })
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .collect();
                    let wanted_links: BTreeSet<_> = expected_links
                        .iter()
                        .map(|link| link.link_path.clone())
                        .collect();
                    if actual_links != wanted_links {
                        errors.push(format!(
                            "{} {} {direction}: missing {:?}; unexpected {:?}",
                            case.name,
                            expectation.path,
                            wanted_links.difference(&actual_links).collect::<Vec<_>>(),
                            actual_links.difference(&wanted_links).collect::<Vec<_>>()
                        ));
                    }
                    for link in expected_links {
                        if actual.contains(&link.link_path) != link.is_in_graph {
                            errors.push(format!(
                                "{} {} {direction} {} membership should be {}",
                                case.name, expectation.path, link.link_path, link.is_in_graph
                            ));
                        }
                    }
                }
            }
        }
    }
    for file in &files {
        let name = file.file_name().unwrap().to_str().unwrap();
        if file.starts_with(&expected_outputs) && name.ends_with(".json") {
            assert!(
                used_outputs.contains(file),
                "Unused expected output: {}",
                file.display()
            );
        }
        if name.contains(".nodespec-") && name.ends_with(".json") {
            assert!(
                used_specs.contains(file),
                "Unused node expectation: {}",
                file.display()
            );
        }
    }
    assert!(assertions > 0, "No fixture expectations were exercised");
    assert!(
        errors.is_empty(),
        "{} query fixture failures:\n{}",
        errors.len(),
        errors.join("\n")
    );
}
