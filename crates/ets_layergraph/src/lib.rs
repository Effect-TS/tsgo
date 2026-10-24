// Package layergraph models how Effect Layers are composed in source code.
// It builds directed graphs where nodes represent Layer expressions and edges
// represent composition relationships (pipe, call, array literal, symbol).
//
// Rust: Go's functions that take the type parser and its checker take the parser only; the checker is the
// parser's (`tp.checker()`), because the parser borrows it mutably. Functions that take only the checker take
// `&mut Checker`.

mod extract;
mod format;
#[cfg(test)]
mod format_test;
mod magic;
#[cfg(test)]
mod magic_test;
mod mermaidurl;
#[cfg(test)]
mod mermaidurl_test;
mod outline;
mod providers;
#[cfg(test)]
mod providers_test;
mod types;
#[cfg(test)]
mod types_test;

pub use extract::*;
pub use format::*;
pub use magic::*;
pub use mermaidurl::*;
pub use outline::*;
pub use providers::*;
pub use types::*;
