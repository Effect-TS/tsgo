// Package directives implements parsing for @effect-diagnostics directive comments.

mod parser;
pub use parser::*;

#[cfg(test)]
mod parser_test;
