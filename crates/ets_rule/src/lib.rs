// Package rule defines the Rule struct for Effect diagnostic rules.

mod context;
mod metadata;
mod rule;
pub use context::*;
pub use metadata::*;
pub use rule::*;

#[cfg(test)]
mod rule_test;
