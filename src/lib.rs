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
pub use graph::Graph;
pub use model::*;
pub use parser::Link;

pub fn query(request: &Request) -> anyhow::Result<Response> {
    Graph::open(&request.source_root, &request.index)?.query(&request.query)
}

#[cfg(test)]
#[path = "../tests/unit/traversal_specs.rs"]
mod traversal_specs;
