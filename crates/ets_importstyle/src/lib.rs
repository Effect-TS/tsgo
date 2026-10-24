// Package importstyle classifies module specifiers against the configured
// namespaceImportPackages and barrelImportPackages options.

mod barrel;
mod importstyle;
pub use barrel::*;
pub use importstyle::*;

#[cfg(test)]
mod barrel_test;
#[cfg(test)]
mod importstyle_test;
