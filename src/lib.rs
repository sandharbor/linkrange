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
    let graph = Graph::open(&request.sources, &request.index)?;
    graph.query(&request.query)
}

#[cfg(test)]
#[path = "../tests/unit/traversal_specs.rs"]
mod traversal_specs;
