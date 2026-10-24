//! Rust-only: Go standard-library equivalents the Rust port of the Effect code needs (`fmt` verbs, `errors`,
//! `path`, `reflect` struct metadata, `strconv`, `strings`, `unicode/utf8`, the tie order of Go's unstable sorts, `testing.T`).
//!
//! There is no Go original in this repository, so no origin markers; every item's doc names the Go API it
//! mirrors.

pub mod errors;
pub mod fmt;
pub mod panics;
pub mod path;
pub mod reflect;
pub mod slices;
pub mod sort;
pub mod strconv;
pub mod strings;
pub mod testing;
pub mod utf8;

pub use errors::Error;
