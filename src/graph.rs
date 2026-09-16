use crate::{index, resolver::Resolver, *};
use anyhow::{Context, Result};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    path::Path,
    time::Instant,
};

pub struct Graph {
    index: index::Index,
    by_path: HashMap<String, usize>,
    outgoing: Vec<Vec<(usize, usize)>>,
    incoming: Vec<Vec<usize>>,
    resolver: Resolver,
}

#[derive(Clone, PartialEq, Eq)]
struct State {
    id: usize,
    out: i64,
    incoming: u32,
    depth: u32,
    route: Vec<usize>,
    via: &'static str,
    embedded: bool,
    inherited: Option<TraversalState>,
    override_out: Option<u32>,
    override_in: Option<u32>,
}

#[derive(Default, Clone)]
struct Policy {
    out: Option<u32>,
    incoming: Option<u32>,
    stop: bool,
    exclude: bool,
}

fn normalize(path: &str) -> Result<String> {
    anyhow::ensure!(
        !path.starts_with('/') && !path.contains('\\') && !path.contains('\0'),
        "Expected source-root-relative path: {path}"
    );
    anyhow::ensure!(
        !path.split('/').any(|segment| segment == ".."),
        "Path must remain inside source root: {path}"
    );
    Ok(path
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect::<Vec<_>>()
        .join("/"))
}

fn display_key(state: &State) -> (u8, i64, u32, i64, std::cmp::Reverse<u32>, &Vec<usize>) {
    let class = if state.out >= 0 {
        0
    } else if state.embedded {
        1
    } else {
        2
    };
    (
        class,
        if class == 2 { -state.out } else { 0 },
        state.depth,
        -state.out,
        std::cmp::Reverse(state.incoming),
        &state.route,
    )
}

fn enqueue(
    candidate: State,
    displays: &mut HashMap<usize, State>,
    states: &mut HashMap<usize, Vec<State>>,
    queue: &mut VecDeque<State>,
) {
    // Keep the shortest valid arrival for presentation even when a longer route
    // has stronger budgets and dominates it for subsequent exploration.
    let display = displays
        .entry(candidate.id)
        .or_insert_with(|| candidate.clone());
    if display_key(&candidate) < display_key(display) {
        *display = candidate.clone();
    }
    let existing = states.entry(candidate.id).or_default();
    if existing.iter().any(|state| {
        state.embedded == candidate.embedded
            && state.out >= candidate.out
            && state.incoming >= candidate.incoming
            && (state.out != candidate.out
                || state.incoming != candidate.incoming
                || (state.route.len(), &state.route) <= (candidate.route.len(), &candidate.route))
    }) {
        return;
    }
    existing.retain(|state| {
        state.embedded != candidate.embedded
            || !(candidate.out >= state.out && candidate.incoming >= state.incoming)
    });
    existing.push(candidate.clone());
    queue.push_back(candidate);
}

impl Graph {
    pub fn open(root: &Path, options: &IndexOptions) -> Result<Self> {
        let root = root.canonicalize().context("Source root is unavailable")?;
        anyhow::ensure!(root.is_dir(), "Source root must be a directory");
        let index = index::load(&root, options)?;
        Self::from_index(index)
    }

    pub(crate) fn from_index(mut index: index::Index) -> Result<Self> {
        let timer = Instant::now();
        let resolver = Resolver::new(
            index.files.iter().map(|entry| &entry.scan.source_file),
            &index.aliases,
        );
        let by_path: HashMap<_, _> = index
            .files
            .iter()
            .enumerate()
            .map(|(id, entry)| (entry.file.path.clone(), id))
            .collect();
        let mut outgoing = vec![Vec::new(); index.files.len()];
        let mut incoming = vec![Vec::new(); index.files.len()];
        for (id, entry) in index.files.iter_mut().enumerate() {
            for (link_id, link) in entry.scan.outgoing_links.iter_mut().enumerate() {
                resolver.resolve(link, &entry.file.directory);
                if let Some(target) = link.target.as_ref().and_then(|path| by_path.get(path)) {
                    outgoing[id].push((*target, link_id));
                    incoming[*target].push(id);
                }
            }
        }
        for adjacent in &mut incoming {
            adjacent.sort_unstable();
            adjacent.dedup();
        }
        index
            .metrics
            .phases_ms
            .insert("resolution".into(), timer.elapsed().as_secs_f64() * 1000.0);
        Ok(Self {
            index,
            by_path,
            outgoing,
            incoming,
            resolver,
        })
    }

    /// Inventory access for native integrations; CLI responses remain query-scoped.
    pub fn files(&self) -> impl Iterator<Item = &FileInfo> {
        self.index.files.iter().map(|entry| &entry.file)
    }
    pub fn directories(&self) -> &BTreeSet<String> {
        &self.index.directories
    }
    pub fn metrics(&self) -> &Metrics {
        &self.index.metrics
    }

    pub fn canonical_path(&self, path: &str) -> Result<String> {
        let path = normalize(path)?;
        let alias = self
            .index
            .aliases
            .iter()
            .filter(|(alias, _)| path == ***alias || path.starts_with(&format!("{alias}/")))
            .max_by_key(|(alias, _)| alias.len());
        Ok(alias.map_or_else(
            || path.clone(),
            |(alias, target)| format!("{target}{}", &path[alias.len()..]),
        ))
    }

    pub fn query(&self, query: &Query) -> Result<Response> {
        anyhow::ensure!(
            !query.starts.is_empty(),
            "At least one file or folder start is required"
        );
        let timer = Instant::now();
        let mut rules = Vec::new();
        for rule in &query.rules {
            rules.push((self.canonical_path(&rule.path)?, rule));
        }
        rules.sort_by_key(|(path, _)| path.len());
        let policy_for = |path: &str| {
            let mut policy = Policy::default();
            for (selector, rule) in &rules {
                if path == selector
                    || (rule.subtree
                        && (selector.is_empty() || path.starts_with(&format!("{selector}/"))))
                {
                    if rule.outlinks.is_some() {
                        policy.out = rule.outlinks;
                    }
                    if rule.inlinks.is_some() {
                        policy.incoming = rule.inlinks;
                    }
                    policy.stop |= rule.stop;
                    policy.exclude |= rule.exclude;
                }
            }
            policy
        };
        let mut policies: HashMap<usize, Policy> = HashMap::new();
        let mut displays = HashMap::new();
        let mut states = HashMap::new();
        let mut queue = VecDeque::new();
        for start in &query.starts {
            let path = self.canonical_path(&start.path)?;
            let seeds: Vec<_> = if let Some(id) = self.by_path.get(&path) {
                vec![*id]
            } else {
                anyhow::ensure!(
                    self.index.directories.contains(&path),
                    "Required starting page or folder is missing: {path}"
                );
                self.index
                    .files
                    .iter()
                    .enumerate()
                    .filter(|(_, entry)| {
                        path.is_empty() || entry.file.path.starts_with(&format!("{path}/"))
                    })
                    .map(|(id, _)| id)
                    .collect()
            };
            let depths = start.depths.as_ref().unwrap_or(&query.depths);
            for id in seeds {
                let policy = policies
                    .entry(id)
                    .or_insert_with(|| policy_for(&self.index.files[id].file.path));
                if policy.exclude {
                    continue;
                }
                enqueue(
                    State {
                        id,
                        out: policy.out.unwrap_or(depths.outlinks) as i64,
                        incoming: policy.incoming.unwrap_or(depths.inlinks),
                        depth: 0,
                        route: vec![id],
                        via: "start",
                        embedded: false,
                        inherited: None,
                        override_out: policy.out,
                        override_in: policy.incoming,
                    },
                    &mut displays,
                    &mut states,
                    &mut queue,
                );
            }
        }
        while let Some(current) = queue.pop_front() {
            if !states
                .get(&current.id)
                .is_some_and(|states| states.contains(&current))
            {
                continue;
            }
            let policy = policies
                .entry(current.id)
                .or_insert_with(|| policy_for(&self.index.files[current.id].file.path));
            if current.embedded || policy.stop || policy.exclude {
                continue;
            }
            let mut visit = |target: usize, via: &'static str, embedded_link: bool| {
                let policy = policies
                    .entry(target)
                    .or_insert_with(|| policy_for(&self.index.files[target].file.path));
                if policy.exclude {
                    return;
                }
                let next_out = current.out - 1;
                let next_in = current.incoming.saturating_sub(1);
                let embedded = current.out == 0
                    && embedded_link
                    && query
                        .boundary_embed_types
                        .contains(&self.index.files[target].file.format);
                if next_out < -(query.frontier_depth as i64) && !embedded {
                    return;
                }
                let normal = next_out >= 0;
                let mut route = current.route.clone();
                route.push(target);
                let inherited = Some(TraversalState {
                    remaining_outlinks: next_out,
                    remaining_inlinks: next_in,
                });
                let override_out = if normal { policy.out } else { None };
                let override_in = if normal { policy.incoming } else { None };
                let candidate = State {
                    id: target,
                    out: override_out.map_or(next_out, |n| n as i64),
                    incoming: override_in.unwrap_or(next_in),
                    depth: current.depth + 1,
                    route,
                    via,
                    embedded: false,
                    inherited,
                    override_out,
                    override_in,
                };
                if next_out >= -(query.frontier_depth as i64) {
                    enqueue(candidate.clone(), &mut displays, &mut states, &mut queue);
                }
                if embedded {
                    enqueue(
                        State {
                            embedded: true,
                            ..candidate
                        },
                        &mut displays,
                        &mut states,
                        &mut queue,
                    );
                }
            };
            for &(target, link) in &self.outgoing[current.id] {
                visit(
                    target,
                    "outlink",
                    self.index.files[current.id].scan.outgoing_links[link].is_embedded,
                );
            }
            if current.incoming > 0 {
                for &source in &self.incoming[current.id] {
                    if !self.outgoing[current.id]
                        .iter()
                        .any(|(target, _)| *target == source)
                    {
                        visit(source, "inlink", false);
                    }
                }
            }
        }
        let ids: BTreeSet<_> = states.keys().copied().collect();
        let mut nodes = Vec::new();
        for &id in &ids {
            let states = &states[&id];
            let display = &displays[&id];
            let inclusion = if display.out >= 0 {
                Inclusion::Traversal
            } else if display.embedded {
                Inclusion::EmbeddedAsset
            } else {
                Inclusion::Frontier
            };
            let mut summaries: Vec<_> = states
                .iter()
                .map(|state| TraversalState {
                    remaining_outlinks: state.out,
                    remaining_inlinks: state.incoming,
                })
                .collect();
            summaries.sort_by_key(|state| {
                (
                    -state.remaining_outlinks,
                    std::cmp::Reverse(state.remaining_inlinks),
                )
            });
            summaries.dedup();
            nodes.push(Node {
                file: self.index.files[id].file.clone(),
                depth: display.depth,
                remaining_outlinks: display.out,
                remaining_inlinks: display.incoming,
                route: display
                    .route
                    .iter()
                    .map(|id| self.index.files[*id].file.path.clone())
                    .collect(),
                via: display.via.into(),
                inclusion,
                states: summaries,
                inherited: display.inherited.clone(),
                overridden_outlinks: display.override_out,
                overridden_inlinks: display.override_in,
            });
        }
        let mut lookup = ids.clone();
        for path in &query.lookup_paths {
            let path = self.canonical_path(path)?;
            let id = self
                .by_path
                .get(&path)
                .with_context(|| format!("Lookup file is missing: {path}"))?;
            lookup.insert(*id);
        }
        let mut links_by_source = BTreeMap::new();
        let mut adjacency = BTreeMap::new();
        let mut diagnostics = self.index.diagnostics.clone();
        for &id in &lookup {
            let entry = &self.index.files[id];
            diagnostics.extend(entry.diagnostics.clone());
            let mut links = entry.scan.outgoing_links.clone();
            if query.explain_resolution {
                for link in &mut links {
                    link.resolution = Some(self.resolver.explain(link, &entry.file.directory));
                }
            }
            links_by_source.insert(entry.file.path.clone(), links);
            if query.adjacency {
                let outlinks: BTreeSet<_> = self.outgoing[id]
                    .iter()
                    .map(|(target, _)| self.index.files[*target].file.path.clone())
                    .collect();
                adjacency.insert(
                    entry.file.path.clone(),
                    Adjacency {
                        inlinks: self.incoming[id]
                            .iter()
                            .map(|id| self.index.files[*id].file.path.clone())
                            .collect(),
                        outlinks: outlinks.into_iter().collect(),
                    },
                );
            }
        }
        let mut edges = Vec::new();
        for &source in &ids {
            for &(target, link) in &self.outgoing[source] {
                if ids.contains(&target) {
                    edges.push(Edge {
                        source: self.index.files[source].file.path.clone(),
                        target: self.index.files[target].file.path.clone(),
                        bidirectional: self.outgoing[target].iter().any(|(id, _)| *id == source),
                        link: self.index.files[source].scan.outgoing_links[link].clone(),
                    });
                }
            }
        }
        let mut metrics = self.index.metrics.clone();
        metrics
            .phases_ms
            .insert("query".into(), timer.elapsed().as_secs_f64() * 1000.0);
        Ok(Response {
            schema_version: SCHEMA_VERSION,
            complete: self.index.complete,
            nodes,
            edges,
            links_by_source,
            adjacency,
            diagnostics,
            metrics,
        })
    }
}
