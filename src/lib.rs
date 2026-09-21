//! Build bounded context graphs from local linked files.
//! See [`Request`], [`Graph::open`], and [`Graph::query`].
mod graph;
mod index;
pub mod links;
mod markup;
mod metadata;
mod model;
mod parser;
mod resolver;
mod sources;
pub use graph::Graph;
pub use model::*;
pub use parser::Link;
pub use sources::{parse_source_locator, source_locator, validate_source_name};

pub fn query(request: &Request) -> anyhow::Result<Response> {
    let graph = match (&request.source_root, &request.sources) {
        (Some(root), None) => Graph::open(root, &request.index)?,
        (None, Some(sources)) => Graph::open_sources(sources, &request.index)?,
        _ => anyhow::bail!("Specify exactly one of sourceRoot or sources"),
    };
    graph.query(&request.query)
}

#[cfg(test)]
#[path = "../tests/unit/traversal_specs.rs"]
mod traversal_specs;
