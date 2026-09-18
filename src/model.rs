use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrontmatterField {
    pub key: String,
    /// Literal prefilter; defaults to the key. Only matching frontmatter is parsed.
    #[serde(default)]
    pub substring: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Symlinks {
    #[default]
    Skip,
    FollowInternal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct IndexOptions {
    pub cache_directory: Option<PathBuf>,
    pub no_cache: bool,
    pub rebuild: bool,
    pub best_effort: bool,
    pub symlinks: Symlinks,
    pub include_hidden: bool,
    pub frontmatter: Vec<FrontmatterField>,
}

impl Default for IndexOptions {
    fn default() -> Self {
        Self {
            cache_directory: None,
            no_cache: false,
            rebuild: false,
            best_effort: false,
            symlinks: Symlinks::Skip,
            include_hidden: false,
            frontmatter: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Depths {
    pub outlinks: u32,
    pub inlinks: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Start {
    pub path: String,
    #[serde(default)]
    pub depths: Option<Depths>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Rule {
    pub path: String,
    pub outlinks: Option<u32>,
    pub inlinks: Option<u32>,
    pub stop: bool,
    pub exclude: bool,
    /// Apply to this directory and its descendants.
    pub subtree: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Query {
    pub starts: Vec<Start>,
    pub depths: Depths,
    pub rules: Vec<Rule>,
    pub frontier_depth: u32,
    pub boundary_embed_types: Vec<String>,
    /// Source formats whose directly embedded files may cross the boundary.
    /// This complements the target-format selection in `boundary_embed_types`.
    pub boundary_embed_source_types: Vec<String>,
    pub adjacency: bool,
    pub lookup_paths: Vec<String>,
    pub explain_resolution: bool,
    /// Include indexing and query measurements in the response.
    pub metrics: bool,
}

impl Default for Query {
    fn default() -> Self {
        Self {
            starts: Vec::new(),
            depths: Depths {
                outlinks: 1,
                inlinks: 0,
            },
            rules: Vec::new(),
            frontier_depth: 0,
            boundary_embed_types: Vec::new(),
            boundary_embed_source_types: Vec::new(),
            adjacency: false,
            lookup_paths: Vec::new(),
            explain_resolution: false,
            metrics: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub source_root: PathBuf,
    #[serde(default)]
    pub index: IndexOptions,
    pub query: Query,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub path: String,
    pub code: String,
    pub message: String,
}

impl Diagnostic {
    pub(crate) fn new(path: &str, code: &str, message: impl ToString) -> Self {
        Self {
            path: path.into(),
            code: code.into(),
            message: message.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub path: String,
    pub format: String,
    pub title: String,
    pub directory: String,
    pub digest: String,
    pub size: u64,
    pub metadata: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Inclusion {
    Traversal,
    Frontier,
    EmbeddedAsset,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TraversalState {
    pub remaining_outlinks: i64,
    pub remaining_inlinks: u32,
}

/// One arrival on a node's selected route, including the budgets used to continue.
/// These values can differ from the same page's independently selected arrival.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RouteStep {
    pub path: String,
    pub depth: u32,
    pub remaining_outlinks: i64,
    pub remaining_inlinks: u32,
    pub via: String,
    pub inclusion: Inclusion,
    pub inherited: Option<TraversalState>,
    pub overridden_outlinks: Option<u32>,
    pub overridden_inlinks: Option<u32>,
    /// Whether this exact arrival is retained in the node's final traversal states.
    /// False means it is retained only as route evidence, even if its budgets equal
    /// another arrival's. This is not a history of which queue entries were expanded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retained_for_traversal: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    #[serde(flatten)]
    pub file: FileInfo,
    pub depth: u32,
    pub remaining_outlinks: i64,
    pub remaining_inlinks: u32,
    pub route: Vec<String>,
    #[serde(default)]
    pub route_steps: Vec<RouteStep>,
    /// Other non-dominated arrivals and strongest inherited budgets replaced by overrides,
    /// each with its own paired budgets and provenance. Override evidence is display-only.
    /// The primary route remains in `route_steps`; maxima must not be merged for traversal.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternative_routes: Vec<Vec<RouteStep>>,
    pub via: String,
    pub inclusion: Inclusion,
    pub states: Vec<TraversalState>,
    pub inherited: Option<TraversalState>,
    pub overridden_outlinks: Option<u32>,
    pub overridden_inlinks: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Adjacency {
    pub inlinks: Vec<String>,
    pub outlinks: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    pub indexed_files: usize,
    pub files_read: usize,
    pub link_parses: usize,
    pub yaml_parses: usize,
    pub cache_bytes: u64,
    pub cache_rebuilt: bool,
    pub phases_ms: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolutionExplanation {
    pub reason: String,
    pub candidates: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub bidirectional: bool,
    pub link: crate::Link,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub schema_version: u32,
    pub complete: bool,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub links_by_source: BTreeMap<String, Vec<crate::Link>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub adjacency: BTreeMap<String, Adjacency>,
    pub diagnostics: Vec<Diagnostic>,
    /// Present only when the query requests metrics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics: Option<Metrics>,
}
