// Package graph provides a generic directed graph data structure for modeling
// typed relationships between nodes.

mod graph;
pub use graph::*;

#[cfg(test)]
mod graph_test;
