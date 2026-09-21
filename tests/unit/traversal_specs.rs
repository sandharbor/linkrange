#![allow(dead_code, clippy::too_many_arguments, irrefutable_let_patterns)]
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct File {
    pub directory: String,
    pub name: String,
    pub file_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default)]
    pub sensitive: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub override_out: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub override_in: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_at: Option<bool>,
}

impl File {
    pub fn key(&self) -> String {
        format!("{}/{}.{}", self.directory, self.name, self.file_type)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Details {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outlinks_depth_set_first_time: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outlinks_depth_inherited: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outlinks_depth_overridden: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inlinks_depth_set_first_time: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inlinks_depth_inherited: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inlinks_depth_overridden: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_type: Option<LinkType>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkType {
    Start,
    Outlink,
    Inlink,
    Bidirectional,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reached {
    pub file: File,
    pub depth: i32,
    pub remaining_depth: i32,
    pub remaining_inlinks_depth: i32,
    pub path: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub traversal_details: Option<Details>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_frontier_node: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_boundary_embed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub traversal_states: Option<Vec<Summary>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    pub remaining_outlinks_depth: i32,
    pub remaining_inlinks_depth: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub is_embedded: bool,
    pub source: File,
    pub target: File,
    pub is_bidirectional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeenEdge {
    pub from: String,
    pub to: String,
    pub bundle_edge_kind: EdgeKind,
    pub is_bidirectional: bool,
    pub is_traversal_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EdgeKind {
    SemanticLink,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(title: &str, file_type: &str) -> File {
        File {
            directory: "".to_string(),
            name: title.to_string(),
            file_type: file_type.to_string(),
            id: None,
            sensitive: false,
            override_out: None,
            override_in: None,
            stop_at: None,
        }
    }

    fn sorted_nodes(nodes: &[Reached]) -> Vec<Reached> {
        let mut out = nodes.to_vec();
        out.sort_by(|a, b| {
            if a.depth != b.depth {
                a.depth.cmp(&b.depth)
            } else {
                a.file.name.cmp(&b.file.name)
            }
        });
        out
    }

    fn name_and_depth(nodes: &[Reached]) -> Vec<String> {
        sorted_nodes(nodes)
            .into_iter()
            .map(|n| format!("{}:{}", n.file.name, n.depth))
            .collect()
    }

    fn name_and_remaining_depth(nodes: &[Reached]) -> Vec<String> {
        sorted_nodes(nodes)
            .into_iter()
            .map(|n| format!("{}:{}", n.file.name, n.remaining_depth))
            .collect()
    }

    fn name_and_remaining_inlinks_depth(nodes: &[Reached]) -> Vec<String> {
        sorted_nodes(nodes)
            .into_iter()
            .map(|n| format!("{}:{}", n.file.name, n.remaining_inlinks_depth))
            .collect()
    }

    fn link_type_str(lt: LinkType) -> &'static str {
        match lt {
            LinkType::Start => "start",
            LinkType::Outlink => "outlink",
            LinkType::Inlink => "inlink",
            LinkType::Bidirectional => "bidirectional",
        }
    }

    fn traversal_details_string(nodes: &[Reached]) -> Vec<String> {
        sorted_nodes(nodes)
            .into_iter()
            .map(|n| {
                let details = n.traversal_details.clone();
                if details.is_none() {
                    return format!("{}: no details", n.file.name);
                }
                let d = details.unwrap();
                let mut parts: Vec<String> = vec![format!("{}:", n.file.name)];
                if let Some(v) = d.outlinks_depth_set_first_time {
                    parts.push(format!("gd_first={}", v));
                }
                if let Some(v) = d.outlinks_depth_inherited {
                    parts.push(format!("gd_inherited={}", v));
                }
                if let Some(v) = d.outlinks_depth_overridden {
                    parts.push(format!("gd_override={}", v));
                }
                if let Some(v) = d.inlinks_depth_set_first_time {
                    parts.push(format!("id_first={}", v));
                }
                if let Some(v) = d.inlinks_depth_inherited {
                    parts.push(format!("id_inherited={}", v));
                }
                if let Some(v) = d.inlinks_depth_overridden {
                    parts.push(format!("id_override={}", v));
                }
                if let Some(lt) = d.link_type {
                    parts.push(format!("link={}", link_type_str(lt)));
                }
                parts.join(" ")
            })
            .collect()
    }

    fn conf(
        name: &str,
        list_type: &str,
        outlinks_depth: Option<i32>,
        inlinks_depth: Option<i32>,
    ) -> Config {
        Config::file(
            name.to_string(),
            None,
            "md".to_string(),
            format!("{:0<12}", name.to_ascii_lowercase()),
            list_type.to_string(),
            outlinks_depth,
            inlinks_depth,
        )
    }

    fn default_confs() -> Vec<Config> {
        vec![conf("A", "whitelist", None, None)]
    }

    #[test]
    fn multi_seed_traversal_retains_non_dominated_budget_states() {
        let a = file("A", "md");
        let b = file("B", "md");
        let d = file("D", "md");
        let e = file("E", "md");
        let edges = vec![
            Edge {
                is_embedded: true,
                source: a.clone(),
                target: b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: b.clone(),
                target: d,
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: e,
                target: b.clone(),
                is_bidirectional: false,
            },
        ];
        // Both budgets now decrease on every hop; the incoming-useful seed needs an outgoing hop too.
        let nodes = get_multi_seed_working_nodes(
            &edges,
            &[],
            &[
                MultiSeed {
                    file: a,
                    outlinks_depth: 3,
                    inlinks_depth: 0,
                    structural_path: vec!["folder:".to_string()],
                },
                MultiSeed {
                    file: b,
                    outlinks_depth: 1,
                    inlinks_depth: 2,
                    structural_path: vec!["folder:".to_string()],
                },
            ],
            &HashSet::new(),
            0,
            false,
        );
        let names: HashSet<&str> = nodes.iter().map(|node| node.file.name.as_str()).collect();
        assert!(names.contains("D"), "outlink-useful state must be explored");
        assert!(names.contains("E"), "inlink-useful state must be explored");
        let b_node = nodes.iter().find(|node| node.file.name == "B").unwrap();
        assert_eq!(
            b_node.traversal_states.as_ref().unwrap(),
            &vec![
                Summary {
                    remaining_outlinks_depth: 2,
                    remaining_inlinks_depth: 0
                },
                Summary {
                    remaining_outlinks_depth: 1,
                    remaining_inlinks_depth: 2
                },
            ]
        );
    }

    #[test]
    fn multi_seed_traversal_discards_dominated_states_and_suppresses_cycles() {
        let a = file("A", "md");
        let b = file("B", "md");
        let edges = vec![
            Edge {
                is_embedded: true,
                source: a.clone(),
                target: b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: b.clone(),
                target: a,
                is_bidirectional: false,
            },
        ];
        let nodes = get_multi_seed_working_nodes(
            &edges,
            &[],
            &[
                MultiSeed {
                    file: b.clone(),
                    outlinks_depth: 2,
                    inlinks_depth: 0,
                    structural_path: vec![],
                },
                MultiSeed {
                    file: b.clone(),
                    outlinks_depth: 0,
                    inlinks_depth: 2,
                    structural_path: vec![],
                },
                MultiSeed {
                    file: b,
                    outlinks_depth: 2,
                    inlinks_depth: 2,
                    structural_path: vec![],
                },
            ],
            &HashSet::new(),
            0,
            false,
        );
        assert_eq!(nodes.len(), 2);
        let b_node = nodes.iter().find(|node| node.file.name == "B").unwrap();
        assert_eq!(
            b_node.traversal_states.as_ref().unwrap(),
            &vec![Summary {
                remaining_outlinks_depth: 2,
                remaining_inlinks_depth: 2
            }]
        );
    }

    #[test]
    fn multi_seed_traversal_omits_folder_blacklisted_seeds_and_their_links() {
        let a = file("A", "md");
        let b = file("B", "md");
        let a_key = a.key();
        let edges = vec![Edge {
            is_embedded: true,
            source: a.clone(),
            target: b,
            is_bidirectional: false,
        }];
        let nodes = get_multi_seed_working_nodes(
            &edges,
            &[],
            &[MultiSeed {
                file: a,
                outlinks_depth: 2,
                inlinks_depth: 0,
                structural_path: vec!["folder:Alpha".to_string()],
            }],
            &HashSet::from([a_key]),
            0,
            false,
        );

        assert!(nodes.is_empty());
    }

    fn default_conf_with_overrides(
        outlinks_depth: Option<i32>,
        inlinks_depth: Option<i32>,
    ) -> Config {
        let mut c = default_confs()[0].clone();
        if let Config::File {
            outlinks_depth: configured_outlinks_depth,
            inlinks_depth: configured_inlinks_depth,
            ..
        } = &mut c
        {
            if outlinks_depth.is_some() {
                *configured_outlinks_depth = outlinks_depth;
            }
            if inlinks_depth.is_some() {
                *configured_inlinks_depth = inlinks_depth;
            }
        }
        c
    }

    fn my_get_working_graph(
        edges: &[Edge],
        configs: &[Config],
        entry_node: &File,
        default_traversal_node: &File,
        allow_lower_depths: bool,
        frontier_depth: i32,
        allow_images_to_extend_to_frontier: bool,
    ) -> (Vec<Reached>, Vec<SeenEdge>) {
        get_working_graph(
            edges,
            configs,
            entry_node,
            default_traversal_node,
            Some(4),
            Some(0),
            TraverseOpts { allow_lower_depths },
            frontier_depth,
            allow_images_to_extend_to_frontier,
        )
        .unwrap()
    }

    fn edge_descriptions(edges: &[SeenEdge], nodes: &[Reached]) -> Vec<String> {
        let id_to_title: HashMap<String, String> = nodes
            .iter()
            .map(|n| (n.file.key(), n.file.name.clone()))
            .collect();
        let mut out: Vec<String> = edges
            .iter()
            .map(|e| {
                let from = id_to_title.get(&e.from).cloned().unwrap_or(e.from.clone());
                let to = id_to_title.get(&e.to).cloned().unwrap_or(e.to.clone());
                if e.is_bidirectional {
                    format!("{}->{} (bi)", from, to)
                } else {
                    format!("{}->{}", from, to)
                }
            })
            .collect();
        out.sort();
        out
    }

    #[test]
    fn building_by_default_does_not_include_inlinks() {
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let node_d = file("D", "md");
        let node_e = file("E", "md");
        let node_f = file("F", "md");
        let node_g = file("G", "md");
        let node_h = file("H", "md");
        let node_i = file("I", "md");
        let node_j = file("J", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_d.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_g.clone(),
                target: node_h.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_h.clone(),
                target: node_i.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_f.clone(),
                target: node_a.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_f.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_g.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_j.clone(),
                target: node_h.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_e.clone(),
                target: node_b.clone(),
                is_bidirectional: true,
            },
        ];

        let confs = default_confs();
        let (nodes, _edges) =
            my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        assert_eq!(
            name_and_depth(&nodes),
            vec!["A:0", "B:1", "D:1", "C:2", "E:2"]
        );
    }

    #[test]
    fn building_includes_bidirectional_inlinks_even_if_inlinks_depth_0() {
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let node_d = file("D", "md");
        let node_e = file("E", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_d.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_e.clone(),
                target: node_b.clone(),
                is_bidirectional: true,
            },
        ];

        let confs = default_confs();
        let (nodes, _edges) =
            my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        assert_eq!(
            name_and_depth(&nodes),
            vec!["A:0", "B:1", "D:1", "C:2", "E:2"]
        );
    }

    #[test]
    fn building_respects_outlinks_depth_for_default_outlinks_only() {
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let node_d = file("D", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
        ];

        let confs = vec![default_conf_with_overrides(Some(1), None)];
        let (nodes, _edges) =
            my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        assert_eq!(name_and_depth(&nodes), vec!["A:0", "B:1", "D:1"]);
    }

    #[test]
    fn building_follows_inlinks_if_inlinks_depth_gt_0() {
        let max_inlinks_depth = 100;
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let node_d = file("D", "md");
        let node_e = file("E", "md");
        let node_f = file("F", "md");
        let node_g = file("G", "md");
        let node_h = file("H", "md");
        let node_i = file("I", "md");
        let node_j = file("J", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_d.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_g.clone(),
                target: node_h.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_h.clone(),
                target: node_i.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_f.clone(),
                target: node_a.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_f.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_g.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_j.clone(),
                target: node_h.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_e.clone(),
                target: node_b.clone(),
                is_bidirectional: true,
            },
        ];

        let confs = vec![default_conf_with_overrides(None, Some(max_inlinks_depth))];
        let (nodes, _edges) =
            my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        let full_listing = vec![
            "A:0", "B:1", "D:1", "F:1", "C:2", "E:2", "G:2", "H:3", "I:4", "J:4",
        ];
        assert_eq!(name_and_depth(&nodes), full_listing);
    }

    #[test]
    fn building_respects_outlinks_depth_with_inlinks() {
        let max_inlinks_depth = 100;
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let node_d = file("D", "md");
        let node_e = file("E", "md");
        let node_f = file("F", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_d.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_f.clone(),
                target: node_a.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_e.clone(),
                target: node_b.clone(),
                is_bidirectional: true,
            },
        ];

        let confs = vec![default_conf_with_overrides(
            Some(1),
            Some(max_inlinks_depth),
        )];
        let (nodes, _edges) =
            my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        assert_eq!(name_and_depth(&nodes), vec!["A:0", "B:1", "D:1", "F:1"]);
    }

    #[test]
    fn override_out_override_allows_deeper_pages_1() {
        let max_inlinks_depth = 100;
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let node_d = file("D", "md");
        let node_e = file("E", "md");
        let node_f = file("F", "md");
        let node_g = file("G", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_d.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_f.clone(),
                target: node_a.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_g.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_e.clone(),
                target: node_b.clone(),
                is_bidirectional: true,
            },
        ];

        let confs: Vec<Config> = vec![
            conf("A", "whitelist", Some(1), Some(max_inlinks_depth)),
            conf("B", "whitelist", Some(1), None),
        ];

        let (nodes, _edges) =
            my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        assert_eq!(
            name_and_depth(&nodes),
            vec!["A:0", "B:1", "D:1", "F:1", "C:2", "E:2", "G:2"]
        );
        assert_eq!(
            name_and_remaining_depth(&nodes),
            vec!["A:1", "B:1", "D:0", "F:0", "C:0", "E:0", "G:0"]
        );
    }

    #[test]
    fn override_out_override_allows_deeper_pages_2() {
        let max_inlinks_depth = 100;
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let node_d = file("D", "md");
        let node_e = file("E", "md");
        let node_f = file("F", "md");
        let node_g = file("G", "md");
        let node_h = file("H", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_d.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_f.clone(),
                target: node_a.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_g.clone(),
                target: node_h.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_g.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_e.clone(),
                target: node_b.clone(),
                is_bidirectional: true,
            },
        ];

        let confs: Vec<Config> = vec![
            conf("A", "whitelist", Some(1), Some(max_inlinks_depth)),
            conf("B", "whitelist", Some(2), None),
        ];

        let (nodes, _edges) =
            my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        assert_eq!(
            name_and_depth(&nodes),
            vec!["A:0", "B:1", "D:1", "F:1", "C:2", "E:2", "G:2", "H:3"]
        );
        assert_eq!(
            name_and_remaining_depth(&nodes),
            vec!["A:1", "B:2", "D:0", "F:0", "C:1", "E:1", "G:1", "H:0"]
        );
    }

    #[test]
    fn override_in_override_can_disable_inlinks_for_deeper_pages() {
        let max_inlinks_depth = 100;
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let node_d = file("D", "md");
        let node_e = file("E", "md");
        let node_f = file("F", "md");
        let node_g = file("G", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_d.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_f.clone(),
                target: node_a.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_g.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_e.clone(),
                target: node_b.clone(),
                is_bidirectional: true,
            },
        ];

        let confs: Vec<Config> = vec![
            conf("A", "whitelist", Some(2), Some(max_inlinks_depth)),
            conf("B", "whitelist", None, Some(0)),
        ];

        let (nodes, _edges) =
            my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        assert_eq!(
            name_and_depth(&nodes),
            vec!["A:0", "B:1", "D:1", "F:1", "C:2", "E:2"]
        );
    }

    #[test]
    fn override_in_override_can_enable_inlinks_for_deeper_pages() {
        let max_inlinks_depth = 100;
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let node_d = file("D", "md");
        let node_e = file("E", "md");
        let node_g = file("G", "md");
        let node_h = file("H", "md");
        let node_i = file("I", "md");
        let node_j = file("J", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_g.clone(),
                target: node_h.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_h.clone(),
                target: node_i.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_g.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_j.clone(),
                target: node_h.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_e.clone(),
                target: node_b.clone(),
                is_bidirectional: true,
            },
        ];

        let confs: Vec<Config> = vec![
            conf("A", "whitelist", Some(1), Some(0)),
            conf("B", "whitelist", Some(3), Some(max_inlinks_depth)),
        ];

        let (nodes, _edges) =
            my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        assert_eq!(
            name_and_depth(&nodes),
            vec!["A:0", "B:1", "D:1", "C:2", "E:2", "G:2", "H:3", "I:4", "J:4"]
        );
    }

    #[test]
    fn inlinks_depth_decreases_by_one_each_level() {
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let node_d = file("D", "md");
        let node_e = file("E", "md");
        let node_f = file("F", "md");
        let node_g = file("G", "md");
        let node_h = file("H", "md");
        let node_i = file("I", "md");
        let node_j = file("J", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_d.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_g.clone(),
                target: node_h.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_h.clone(),
                target: node_i.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_f.clone(),
                target: node_a.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_g.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_j.clone(),
                target: node_h.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_e.clone(),
                target: node_b.clone(),
                is_bidirectional: true,
            },
        ];

        let confs: Vec<Config> = vec![conf("A", "whitelist", Some(4), Some(2))];

        let (nodes, _edges) =
            my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        assert_eq!(
            name_and_depth(&nodes),
            vec!["A:0", "B:1", "D:1", "F:1", "C:2", "E:2", "G:2", "H:3", "I:4"]
        );
    }

    #[test]
    fn inlinks_depth_of_one_only_allows_inlinks_at_same_depth() {
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let node_d = file("D", "md");
        let node_e = file("E", "md");
        let node_f = file("F", "md");
        let node_g = file("G", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_f.clone(),
                target: node_a.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_g.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_e.clone(),
                target: node_b.clone(),
                is_bidirectional: true,
            },
        ];

        let confs: Vec<Config> = vec![conf("A", "whitelist", Some(4), Some(1))];

        let (nodes, _edges) =
            my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        assert_eq!(
            name_and_depth(&nodes),
            vec!["A:0", "B:1", "D:1", "F:1", "C:2", "E:2"]
        );
        assert_eq!(
            name_and_remaining_inlinks_depth(&nodes),
            vec!["A:1", "B:0", "D:0", "F:0", "C:0", "E:0"]
        );
    }

    #[test]
    fn remaining_depth_tracked_depth_two() {
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let node_d = file("D", "md");
        let node_e = file("E", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_e.clone(),
                target: node_b.clone(),
                is_bidirectional: true,
            },
        ];

        let confs: Vec<Config> = vec![conf("A", "whitelist", Some(2), Some(0))];

        let (nodes, _edges) =
            my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        assert_eq!(
            name_and_remaining_depth(&nodes),
            vec!["A:2", "B:1", "D:1", "C:0", "E:0"]
        );
    }

    #[test]
    fn remaining_depth_tracked_depth_one() {
        let max_inlinks_depth = 100;
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_d = file("D", "md");
        let node_f = file("F", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_f.clone(),
                target: node_a.clone(),
                is_bidirectional: false,
            },
        ];

        let confs: Vec<Config> = vec![conf("A", "whitelist", Some(1), Some(max_inlinks_depth))];

        let (nodes, _edges) =
            my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        assert_eq!(
            name_and_remaining_depth(&nodes),
            vec!["A:1", "B:0", "D:0", "F:0"]
        );
    }

    #[test]
    fn depth_limiting_respects_outlinks_depth_limit() {
        let max_inlinks_depth = 100;
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let node_d = file("D", "md");
        let node_e = file("E", "md");
        let node_f = file("F", "md");
        let node_g = file("G", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_f.clone(),
                target: node_a.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_g.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_e.clone(),
                target: node_b.clone(),
                is_bidirectional: true,
            },
        ];

        let confs: Vec<Config> = vec![conf("A", "whitelist", Some(2), Some(max_inlinks_depth))];

        let (nodes, _edges) =
            my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        assert_eq!(
            name_and_remaining_depth(&nodes),
            vec!["A:2", "B:1", "D:1", "F:1", "C:0", "E:0", "G:0"]
        );
    }

    #[test]
    fn blacklisted_pages_included_but_cutoff() {
        let max_inlinks_depth = 100;
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let node_d = file("D", "md");
        let node_f = file("F", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_f.clone(),
                target: node_a.clone(),
                is_bidirectional: false,
            },
        ];

        let confs: Vec<Config> = vec![
            conf("A", "whitelist", Some(2), Some(max_inlinks_depth)),
            conf("B", "blacklist", None, None),
        ];

        let (nodes, _edges) =
            my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        assert_eq!(name_and_depth(&nodes), vec!["A:0", "B:1", "D:1", "F:1"]);
    }

    #[test]
    fn blacklisted_pages_do_not_extend_depth_via_override_out() {
        let max_inlinks_depth = 100;
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let node_d = file("D", "md");
        let node_e = file("E", "md");
        let node_f = file("F", "md");
        let node_g = file("G", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_f.clone(),
                target: node_a.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_g.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_e.clone(),
                target: node_b.clone(),
                is_bidirectional: true,
            },
        ];

        let confs: Vec<Config> = vec![
            conf("A", "whitelist", Some(1), Some(max_inlinks_depth)),
            conf("B", "blacklist", Some(2), None),
        ];

        let (nodes, _) = my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        // B is blacklisted; its outlinks_depth override must not pull in C/E/G.
        assert_eq!(name_and_depth(&nodes), vec!["A:0", "B:1", "D:1", "F:1"]);
    }

    #[test]
    fn traverse_can_start_from_different_points() {
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let node_d = file("D", "md");
        let node_e = file("E", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_d.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_e.clone(),
                target: node_b.clone(),
                is_bidirectional: true,
            },
        ];

        let confs = default_confs();

        let (pages_from_a, _) =
            my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        assert_eq!(
            name_and_depth(&pages_from_a),
            vec!["A:0", "B:1", "D:1", "C:2", "E:2"]
        );

        let (pages_from_b, _) =
            my_get_working_graph(&edges, &confs, &node_a, &node_b, false, 0, true);
        assert_eq!(name_and_depth(&pages_from_b), vec!["B:1", "C:2", "E:2"]);
    }

    #[test]
    fn traverse_should_only_traverse_to_same_or_greater_depth_by_default() {
        let max_inlinks_depth = 100;
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_g = file("G", "md");
        let node_h = file("H", "md");
        let node_i = file("I", "md");
        let node_j = file("J", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_g.clone(),
                target: node_h.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_h.clone(),
                target: node_i.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_g.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_j.clone(),
                target: node_h.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
        ];

        let confs = vec![default_conf_with_overrides(None, Some(max_inlinks_depth))];
        let (nodes, _) = my_get_working_graph(&edges, &confs, &node_a, &node_g, false, 0, true);
        assert_eq!(name_and_depth(&nodes), vec!["G:2", "H:3", "I:4", "J:4"]);
    }

    #[test]
    fn traverse_should_traverse_to_all_pages_if_allow_lower_depths_true() {
        let max_inlinks_depth = 100;
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let node_d = file("D", "md");
        let node_e = file("E", "md");
        let node_f = file("F", "md");
        let node_g = file("G", "md");
        let node_h = file("H", "md");
        let node_i = file("I", "md");
        let node_j = file("J", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_d.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_g.clone(),
                target: node_h.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_h.clone(),
                target: node_i.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_f.clone(),
                target: node_a.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_f.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_g.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_j.clone(),
                target: node_h.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_e.clone(),
                target: node_b.clone(),
                is_bidirectional: true,
            },
        ];

        let confs = vec![default_conf_with_overrides(None, Some(max_inlinks_depth))];
        let (nodes, _) = my_get_working_graph(&edges, &confs, &node_a, &node_g, true, 0, true);
        let full_listing = vec![
            "A:0", "B:1", "D:1", "F:1", "C:2", "E:2", "G:2", "H:3", "I:4", "J:4",
        ];
        assert_eq!(name_and_depth(&nodes), full_listing);
    }

    #[test]
    fn edge_dedup_does_not_create_duplicate_pairs() {
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let node_d = file("D", "md");
        let node_e = file("E", "md");
        let node_f = file("F", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_f.clone(),
                target: node_a.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_e.clone(),
                target: node_b.clone(),
                is_bidirectional: true,
            },
        ];

        let confs: Vec<Config> = vec![conf("A", "whitelist", Some(2), Some(1))];

        let (nodes, result_edges) =
            my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        let descriptions = edge_descriptions(&result_edges, &nodes);

        let mut undirected_pairs: HashSet<String> = HashSet::new();
        for desc in descriptions {
            let parts: Vec<&str> = desc
                .split_whitespace()
                .next()
                .unwrap()
                .split("->")
                .collect();
            if parts.len() == 2 {
                let mut a = parts[0].to_string();
                let mut b = parts[1].to_string();
                if a > b {
                    std::mem::swap(&mut a, &mut b);
                }
                let key = format!("{}-{}", a, b);
                assert!(!undirected_pairs.contains(&key));
                undirected_pairs.insert(key);
            }
        }
    }

    #[test]
    fn edge_marked_bidirectional_when_raw_edge_is_bidirectional() {
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let node_d = file("D", "md");
        let node_e = file("E", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
            // Raw bidirectional edge (purposefully backwards)
            Edge {
                is_embedded: true,
                source: node_e.clone(),
                target: node_b.clone(),
                is_bidirectional: true,
            },
        ];

        let confs: Vec<Config> = vec![conf("A", "whitelist", Some(2), Some(0))];

        let (nodes, result_edges) =
            my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        let descriptions = edge_descriptions(&result_edges, &nodes);
        assert!(descriptions
            .iter()
            .any(|d| d == "B->E (bi)" || d == "E->B (bi)"));
    }

    #[test]
    fn edge_keeps_unidirectional_edges_as_non_bidirectional() {
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_d = file("D", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
        ];

        let confs: Vec<Config> = vec![conf("A", "whitelist", Some(2), Some(0))];

        let (nodes, result_edges) =
            my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        let descriptions = edge_descriptions(&result_edges, &nodes);
        assert!(descriptions.contains(&"A->B".to_string()));
        assert!(descriptions.contains(&"A->D".to_string()));
        assert!(!descriptions.contains(&"A->B (bi)".to_string()));
        assert!(!descriptions.contains(&"A->D (bi)".to_string()));
    }

    #[test]
    fn edge_does_not_become_bidirectional_due_to_inlink_traversal_only_reverse() {
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_d = file("D", "md");
        let node_f = file("F", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_f.clone(),
                target: node_a.clone(),
                is_bidirectional: false,
            }, // inlink to A
        ];

        let confs: Vec<Config> = vec![conf("A", "whitelist", Some(2), Some(1))];

        let (nodes, result_edges) =
            my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        let descriptions = edge_descriptions(&result_edges, &nodes);
        // There should be an edge between F and A, but it must not be bidirectional.
        assert!(descriptions.iter().any(|d| d == "F->A" || d == "A->F"));
        assert!(!descriptions
            .iter()
            .any(|d| d == "F->A (bi)" || d == "A->F (bi)"));
    }

    #[test]
    fn traversal_details_are_tracked() {
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let node_d = file("D", "md");
        let node_e = file("E", "md");
        let node_f = file("F", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_f.clone(),
                target: node_a.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_e.clone(),
                target: node_b.clone(),
                is_bidirectional: true,
            },
        ];

        let confs: Vec<Config> = vec![
            conf("A", "whitelist", None, None),
            conf("B", "whitelist", Some(1), Some(0)),
        ];

        let (nodes, _) = get_working_graph(
            &edges,
            &confs,
            &node_a,
            &node_a,
            Some(2),
            Some(1),
            TraverseOpts {
                allow_lower_depths: false,
            },
            0,
            true,
        )
        .unwrap();
        assert_eq!(
            name_and_depth(&nodes),
            vec!["A:0", "B:1", "D:1", "F:1", "C:2", "E:2"]
        );
        assert_eq!(
            traversal_details_string(&nodes),
            vec![
                "A: gd_first=2 id_first=1 link=start",
                "B: gd_inherited=1 gd_override=1 id_inherited=0 id_override=0 link=outlink",
                "D: gd_inherited=1 id_inherited=0 link=outlink",
                "F: gd_inherited=1 id_inherited=0 link=inlink",
                "C: gd_inherited=0 id_inherited=0 link=outlink",
                "E: gd_inherited=0 id_inherited=0 link=bidirectional"
            ]
        );
    }

    #[test]
    fn does_not_process_inlinks_when_remaining_inlinks_depth_is_zero() {
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let node_d = file("D", "md");
        let node_e = file("E", "md");
        let node_f = file("F", "md");
        let node_g = file("G", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_f.clone(),
                target: node_a.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_g.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_e.clone(),
                target: node_b.clone(),
                is_bidirectional: true,
            },
        ];

        let confs: Vec<Config> = vec![conf("A", "whitelist", Some(4), Some(0))];

        let (nodes, _) = my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        assert_eq!(
            name_and_depth(&nodes),
            vec!["A:0", "B:1", "D:1", "C:2", "E:2"]
        );
        assert_eq!(
            name_and_remaining_inlinks_depth(&nodes),
            vec!["A:0", "B:0", "D:0", "C:0", "E:0"]
        );
    }

    #[test]
    fn frontier_image_extension_includes_images_at_frontier_edge_when_enabled() {
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let img = file("IMG", "png");
        let md_link = file("MD_LINK", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_c.clone(),
                target: img.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_c.clone(),
                target: md_link.clone(),
                is_bidirectional: false,
            },
        ];

        let confs: Vec<Config> = vec![conf("A", "whitelist", Some(2), Some(0))];

        let (nodes, _) = my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        assert_eq!(name_and_depth(&nodes), vec!["A:0", "B:1", "C:2", "IMG:3"]);

        let img_page = nodes.iter().find(|p| p.file.name == "IMG").unwrap();
        assert_eq!(img_page.is_boundary_embed, Some(true));
        assert_eq!(img_page.is_frontier_node, Some(false));
    }

    #[test]
    fn linked_image_has_no_embedded_asset_exception() {
        let page = file("A", "md");
        let embedded = file("EMBEDDED", "png");
        let linked = file("LINKED", "png");
        let edges = vec![
            Edge {
                source: page.clone(),
                target: embedded,
                is_bidirectional: false,
                is_embedded: true,
            },
            Edge {
                source: page.clone(),
                target: linked,
                is_bidirectional: false,
                is_embedded: false,
            },
        ];
        let configs = vec![conf("A", "whitelist", Some(0), Some(0))];
        let (included, _) = my_get_working_graph(&edges, &configs, &page, &page, false, 0, true);
        assert_eq!(name_and_depth(&included), vec!["A:0", "EMBEDDED:1"]);
        let (frontier, _) = my_get_working_graph(&edges, &configs, &page, &page, false, 1, true);
        assert!(frontier
            .iter()
            .find(|node| node.file.name == "LINKED")
            .unwrap()
            .is_frontier_node
            .unwrap());
    }

    #[test]
    fn frontier_image_extension_excludes_images_when_disabled() {
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let img = file("IMG", "png");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_c.clone(),
                target: img.clone(),
                is_bidirectional: false,
            },
        ];

        let confs: Vec<Config> = vec![conf("A", "whitelist", Some(2), Some(0))];

        let (nodes, _) = my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, false);
        assert_eq!(name_and_depth(&nodes), vec!["A:0", "B:1", "C:2"]);
    }

    #[test]
    fn frontier_image_extension_does_not_extend_beyond_one_level_past_frontier() {
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let img = file("IMG", "png");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_c.clone(),
                target: img.clone(),
                is_bidirectional: false,
            },
        ];

        let confs: Vec<Config> = vec![conf("A", "whitelist", Some(1), Some(0))];

        let (nodes, _) = my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        assert_eq!(name_and_depth(&nodes), vec!["A:0", "B:1"]);
    }

    #[test]
    fn frontier_image_extension_includes_multiple_images_at_frontier_edge() {
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let img1 = file("IMG", "png");
        let img2 = file("IMG2", "jpg");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_c.clone(),
                target: img1.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_c.clone(),
                target: img2.clone(),
                is_bidirectional: false,
            },
        ];

        let confs: Vec<Config> = vec![conf("A", "whitelist", Some(2), Some(0))];

        let (nodes, _) = my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        assert_eq!(
            name_and_depth(&nodes),
            vec!["A:0", "B:1", "C:2", "IMG:3", "IMG2:3"]
        );

        let img1_page = nodes.iter().find(|p| p.file.name == "IMG").unwrap();
        let img2_page = nodes.iter().find(|p| p.file.name == "IMG2").unwrap();
        assert_eq!(img1_page.is_boundary_embed, Some(true));
        assert_eq!(img2_page.is_boundary_embed, Some(true));
    }

    #[test]
    fn frontier_image_extension_does_not_mark_normal_images_as_extension() {
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let img = file("IMG", "png");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_c.clone(),
                target: img.clone(),
                is_bidirectional: false,
            },
        ];

        let confs: Vec<Config> = vec![conf("A", "whitelist", Some(4), Some(0))];

        let (nodes, _) = my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        let img_page = nodes.iter().find(|p| p.file.name == "IMG").unwrap();
        assert_ne!(img_page.is_boundary_embed, Some(true));
    }

    #[test]
    fn override_in_can_override_twice() {
        let max_inlinks_depth = 100;
        let node_a = file("A", "md");
        let node_b = file("B", "md");
        let node_c = file("C", "md");
        let node_d = file("D", "md");
        let node_e = file("E", "md");
        let node_g = file("G", "md");
        let node_h = file("H", "md");
        let node_i = file("I", "md");
        let node_j = file("J", "md");

        let edges: Vec<Edge> = vec![
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_a.clone(),
                target: node_d.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_b.clone(),
                target: node_c.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_g.clone(),
                target: node_h.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_h.clone(),
                target: node_i.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_g.clone(),
                target: node_b.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_j.clone(),
                target: node_h.clone(),
                is_bidirectional: false,
            },
            Edge {
                is_embedded: true,
                source: node_e.clone(),
                target: node_b.clone(),
                is_bidirectional: true,
            },
        ];

        let confs: Vec<Config> = vec![
            conf("A", "whitelist", Some(1), Some(0)),
            conf("B", "whitelist", Some(3), Some(max_inlinks_depth)),
            conf("G", "whitelist", None, Some(0)),
        ];

        let (nodes, _) = my_get_working_graph(&edges, &confs, &node_a, &node_a, false, 0, true);
        // J should not be included because G's override_in is 0.
        assert_eq!(
            name_and_depth(&nodes),
            vec!["A:0", "B:1", "D:1", "C:2", "E:2", "G:2", "H:3", "I:4"]
        );
    }
}
#[derive(Clone)]
enum Config {
    File {
        name: String,
        directory: Option<String>,
        format: String,
        id: String,
        list_type: String,
        outlinks_depth: Option<i32>,
        inlinks_depth: Option<i32>,
    },
}
impl Config {
    fn file(
        name: String,
        directory: Option<String>,
        format: String,
        id: String,
        list_type: String,
        outlinks_depth: Option<i32>,
        inlinks_depth: Option<i32>,
    ) -> Self {
        Self::File {
            name,
            directory,
            format,
            id,
            list_type,
            outlinks_depth,
            inlinks_depth,
        }
    }
    fn key(&self) -> String {
        let Self::File {
            name,
            directory,
            format,
            ..
        } = self;
        format!("{}/{}.{}", directory.as_deref().unwrap_or(""), name, format)
    }
}
struct TraverseOpts {
    allow_lower_depths: bool,
}
struct MultiSeed {
    file: File,
    outlinks_depth: i32,
    inlinks_depth: i32,
    structural_path: Vec<String>,
}
fn get_multi_seed_working_nodes(
    edges: &[Edge],
    configs: &[Config],
    seeds: &[MultiSeed],
    blocked: &HashSet<String>,
    frontier: i32,
    images: bool,
) -> Vec<Reached> {
    run(
        edges, configs, seeds, blocked, frontier, images, None, false,
    )
    .0
}
fn get_working_graph(
    edges: &[Edge],
    configs: &[Config],
    entry: &File,
    focus: &File,
    out: Option<i32>,
    incoming: Option<i32>,
    options: TraverseOpts,
    frontier: i32,
    images: bool,
) -> anyhow::Result<(Vec<Reached>, Vec<SeenEdge>)> {
    Ok(run(
        edges,
        configs,
        &[MultiSeed {
            file: entry.clone(),
            outlinks_depth: out.unwrap_or(i32::MAX),
            inlinks_depth: incoming.unwrap_or(0),
            structural_path: vec![entry.key()],
        }],
        &HashSet::new(),
        frontier,
        images,
        Some(focus),
        options.allow_lower_depths,
    ))
}
fn run(
    edges: &[Edge],
    configs: &[Config],
    seeds: &[MultiSeed],
    blocked: &HashSet<String>,
    frontier: i32,
    images: bool,
    focus: Option<&File>,
    lower: bool,
) -> (Vec<Reached>, Vec<SeenEdge>) {
    let mut files = BTreeMap::new();
    for edge in edges {
        files.insert(edge.source.key(), edge.source.clone());
        files.insert(edge.target.key(), edge.target.clone());
    }
    for seed in seeds {
        files.insert(seed.file.key(), seed.file.clone());
    }
    let mut records = Vec::new();
    for (key, file) in &files {
        let path = key.trim_start_matches('/').to_string();
        let mut outgoing = Vec::new();
        for edge in edges {
            let target = if edge.source.key() == *key {
                Some(&edge.target)
            } else if edge.is_bidirectional && edge.target.key() == *key {
                Some(&edge.source)
            } else {
                None
            };
            if let Some(target) = target {
                outgoing.push(crate::Link {
                    link_original_text: target.key(),
                    link_source_page_path: path.clone(),
                    link_parsed_directory: target.directory.clone(),
                    link_parsed_title: target.name.clone(),
                    link_parsed_file_type: target.file_type.clone(),
                    link_parsed_anchor: None,
                    link_parsed_anchor_type: None,
                    link_parsed_alias: None,
                    link_parsed_media_size: None,
                    link_resolved_target_directory: String::new(),
                    link_resolved_target_path: String::new(),
                    is_relative_path_link: false,
                    is_embedded: edge.is_embedded,
                    target: None,
                    resolution: None,
                    link_source_name: None,
                    link_requested_target_source: None,
                    link_resolved_target_source: None,
                    link_source_error: None,
                });
            }
        }
        let info = crate::FileInfo {
            path: path.clone(),
            directory: file.directory.clone(),
            title: file.name.clone(),
            format: file.file_type.clone(),
            digest: String::new(),
            size: 0,
            metadata: BTreeMap::new(),
        };
        let identifier = crate::parser::FileIdentifier {
            directory: file.directory.clone(),
            title: file.name.clone(),
            file_type: file.file_type.clone(),
            path,
        };
        records.push(crate::index::Record {
            stamp: Default::default(),
            file: info,
            scan: crate::parser::ScanResult {
                source_file: identifier,
                outgoing_links: outgoing,
            },
            diagnostics: Vec::new(),
        });
    }
    let graph = crate::Graph::from_index(crate::index::Index {
        files: records,
        directories: BTreeSet::new(),
        aliases: BTreeMap::new(),
        diagnostics: Vec::new(),
        complete: true,
        metrics: Default::default(),
    })
    .unwrap();
    let mut rules = Vec::new();
    for config in configs {
        let Config::File {
            list_type,
            outlinks_depth,
            inlinks_depth,
            ..
        } = config;
        rules.push(crate::Rule {
            path: config.key().trim_start_matches('/').into(),
            outlinks: outlinks_depth.map(|n| n as u32),
            inlinks: inlinks_depth.map(|n| n as u32),
            stop: list_type == "blacklist",
            ..Default::default()
        });
    }
    for path in blocked {
        rules.push(crate::Rule {
            path: path.trim_start_matches('/').into(),
            exclude: true,
            ..Default::default()
        });
    }
    let query = crate::Query {
        starts: seeds
            .iter()
            .map(|seed| crate::Start {
                path: seed.file.key().trim_start_matches('/').into(),
                depths: Some(crate::Depths {
                    outlinks: seed.outlinks_depth as u32,
                    inlinks: seed.inlinks_depth as u32,
                }),
            })
            .collect(),
        rules,
        frontier_depth: frontier as u32,
        boundary_embed_types: if images {
            vec!["png".into(), "jpg".into(), "jpeg".into(), "gif".into()]
        } else {
            Vec::new()
        },
        ..Default::default()
    };
    let response = graph.query(&query).unwrap();
    let legacy_key = |path: &str| {
        if path.contains('/') {
            path.into()
        } else {
            format!("/{path}")
        }
    };
    let mut selected: HashSet<_> = response
        .nodes
        .iter()
        .map(|node| node.file.path.clone())
        .collect();
    if let Some(focus) = focus.filter(|focus| {
        seeds
            .first()
            .is_some_and(|seed| seed.file.key() != focus.key())
    }) {
        let focus = focus.key().trim_start_matches('/').to_string();
        let min = response
            .nodes
            .iter()
            .find(|node| node.file.path == focus)
            .map(|node| node.depth)
            .unwrap_or(u32::MAX);
        selected.clear();
        let mut pending = vec![focus];
        while let Some(path) = pending.pop() {
            let Some(node) = response.nodes.iter().find(|node| node.file.path == path) else {
                continue;
            };
            if (!lower && node.depth < min) || !selected.insert(path.clone()) {
                continue;
            }
            for edge in &response.edges {
                if edge.source == path {
                    pending.push(edge.target.clone());
                } else if (lower || node.states.iter().any(|state| state.remaining_inlinks > 0))
                    && edge.target == path
                {
                    pending.push(edge.source.clone());
                }
            }
        }
    }
    let mut nodes = Vec::new();
    for node in response
        .nodes
        .iter()
        .filter(|node| selected.contains(&node.file.path))
    {
        let key = legacy_key(&node.file.path);
        let mut file = files[&key].clone();
        for config in configs {
            if config.key() == key {
                let Config::File {
                    id,
                    list_type,
                    outlinks_depth,
                    inlinks_depth,
                    ..
                } = config;
                file.id = Some(id.clone());
                file.override_out = *outlinks_depth;
                file.override_in = *inlinks_depth;
                file.stop_at = Some(list_type == "blacklist");
            }
        }
        let start = seeds.iter().find(|seed| seed.file.key() == key);
        let reverse_bidirectional = node.route.len() > 1
            && edges.iter().any(|edge| {
                edge.is_bidirectional
                    && edge.source.key() == key
                    && edge.target.key() == legacy_key(&node.route[node.route.len() - 2])
            });
        let details = Details {
            outlinks_depth_set_first_time: node
                .inherited
                .is_none()
                .then(|| start.unwrap().outlinks_depth),
            outlinks_depth_inherited: node
                .inherited
                .as_ref()
                .map(|state| state.remaining_outlinks as i32),
            outlinks_depth_overridden: if node.inherited.is_some() {
                node.overridden_outlinks.map(|n| n as i32)
            } else {
                None
            },
            inlinks_depth_set_first_time: node
                .inherited
                .is_none()
                .then(|| start.unwrap().inlinks_depth),
            inlinks_depth_inherited: node
                .inherited
                .as_ref()
                .map(|state| state.remaining_inlinks as i32),
            inlinks_depth_overridden: if node.inherited.is_some() {
                node.overridden_inlinks.map(|n| n as i32)
            } else {
                None
            },
            link_type: Some(match node.via.as_str() {
                "start" => LinkType::Start,
                "inlink" => LinkType::Inlink,
                _ if reverse_bidirectional => LinkType::Bidirectional,
                _ => LinkType::Outlink,
            }),
        };
        nodes.push(Reached {
            file,
            depth: node.depth as i32,
            remaining_depth: node.remaining_outlinks as i32,
            remaining_inlinks_depth: node.remaining_inlinks as i32,
            path: node.route.iter().map(|path| legacy_key(path)).collect(),
            traversal_details: Some(details),
            is_frontier_node: Some(node.inclusion == crate::Inclusion::Frontier),
            is_boundary_embed: Some(node.inclusion == crate::Inclusion::EmbeddedAsset),
            traversal_states: Some(
                node.states
                    .iter()
                    .map(|state| Summary {
                        remaining_outlinks_depth: state.remaining_outlinks as i32,
                        remaining_inlinks_depth: state.remaining_inlinks as i32,
                    })
                    .collect(),
            ),
        });
    }
    let mut seen = HashSet::new();
    let mut result_edges = Vec::new();
    for edge in &response.edges {
        if !selected.contains(&edge.source) || !selected.contains(&edge.target) {
            continue;
        }
        let from = legacy_key(&edge.source);
        let to = legacy_key(&edge.target);
        let pair = if edge.bidirectional && from > to {
            (to.clone(), from.clone())
        } else {
            (from.clone(), to.clone())
        };
        if seen.insert(pair) {
            result_edges.push(SeenEdge {
                from,
                to,
                bundle_edge_kind: EdgeKind::SemanticLink,
                is_bidirectional: edge.bidirectional,
                is_traversal_only: false,
            });
        }
    }
    (nodes, result_edges)
}
