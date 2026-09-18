//! Query results are checked against authored fixture expectations, never engine output.
use linkrange::*;
use serde::Deserialize;
use std::{collections::BTreeSet, fs, path::Path};

#[derive(Deserialize)]
struct Case {
    name: String,
    source: String,
    query: Query,
    expectations: Option<Vec<Expectation>>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Expectation {
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

#[test]
fn query_results_match_fixture_expectations() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let cases: Vec<Case> =
        serde_json::from_slice(&fs::read(fixture.join("cases.json")).unwrap()).unwrap();
    let cache = tempfile::tempdir().unwrap();
    let mut errors = Vec::new();
    let mut assertions = 0;
    for case in cases {
        let graph = Graph::open(
            &fixture.join(&case.source),
            &IndexOptions {
                cache_directory: Some(cache.path().into()),
                ..Default::default()
            },
        )
        .unwrap();
        let normal = graph.query(&case.query).unwrap();
        let Some(expected) = case.expectations else {
            assert!(normal.complete, "{}", case.name);
            continue;
        };
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
    assert!(assertions > 0, "No fixture expectations were exercised");
    assert!(
        errors.is_empty(),
        "{} query fixture failures:\n{}",
        errors.len(),
        errors.join("\n")
    );
}
