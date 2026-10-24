//! Rust-only: the counterpart of the Go shim package `shim/fourslash` (typescript-go's `internal/fourslash` with
//! effect-tsgo's `_patches/typescript/010-fourslash-fourslash.patch`), reduced to what the Effect language-service
//! baseline runners (`ets_effecttest`) and the fourslash hook (`ets_etstesthooks`) use.
//!
//! tsrs has no Rust fourslash library (its fourslash suite runs typescript-go's Go harness against tsrs server
//! processes, `tsrs/tools/go-fourslash`). This crate ports the pieces of Go's harness the Effect runners call, over
//! tsrs's in-process LSP client (`tsrs_lsp::lsptestutil`, reached through `ets_shim::lsp`), so the requests take the
//! same path through the language server as Go's. There is no Go original in this repository, so no origin
//! markers; every item's doc names the typescript-go (or patch) item it ports. See `docs/TSRS_API.md`, "Gaps".

mod fourslash;
mod test_parser;

pub use fourslash::*;
pub use test_parser::*;
