// Package etsgoapi exposes narrow public entry points for Go integrations that need
// stable access to tsgo functionality without importing internal packages.
//
// What callers must know:
//
// - `TypeParser<'c>` borrows the checker mutably (`new_type_parser(program, &mut checker)`), so Go code that uses
//   the parser and its checker side by side (`typeChecker.TypeToStringEx(...)` next to `typeParser.X(...)`) reads
//   the checker through `tp.checker()`.
// - The type parser's caches (`ets_typeparser`'s `EffectLinks`) live on the checker (`Checker::effect_links`, as
//   Go's `c.EffectLinks`) and are dropped with it; nothing needs releasing.
// - The only `recover` boundary below this package is `ets_typeparser`'s `GetTypeAtLocation` (a `catch_unwind`;
//   the default panic hook still prints the panic to stderr, Go's recover is silent). Neither etsgoapi nor
//   layergraph has one of its own.

mod layer_magic;
mod piping_flow;
mod structural_schema;
mod type_parser;

pub use layer_magic::*;
pub use piping_flow::*;
pub use type_parser::*;
