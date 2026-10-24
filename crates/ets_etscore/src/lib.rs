// Package etscore provides core types for Effect TypeScript integration.
// This is a leaf package with no dependencies, designed to be imported by
// both typescript-go/internal/core and effectapi without circular imports.

mod climode;
mod consts;
mod options;
mod options_parser;
mod severity;
mod version_generated;

pub use climode::*;
pub use consts::*;
pub use options::*;
pub use options_parser::*;
pub use severity::*;
pub use version_generated::*;

#[cfg(test)]
mod climode_test;
#[cfg(test)]
mod severity_test;
#[cfg(test)]
mod version_generated_test;
