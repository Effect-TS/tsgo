//! Rust-only: the effect-tsgo `tsrs-lint` executable, the counterpart of the patched tsgolint
//! (`_patches/tsgolint/001-effect-rules.patch` and the `effect_rules_generated.go` that `repoctl codegen tsgolint`
//! writes).
//!
//! Go appends one `rule.Rule` per Effect rule, named `effecttsgo/<kebab-case name>`, to tsgolint's `allRules`. Each
//! runs `etsoxlintrunner.RunRuleAndReport` once per file and listens to no node. Here the same adapters are built on
//! demand by the factory registered with tsrs's linter, then tsrs's headless `main` runs.

#[cfg(feature = "cli")]
mod adapter;
mod names;

#[cfg(feature = "cli")]
fn main() {
    ets_etsoxlintrunner::init();
    tsrs_lint::registry::register_external_rules(adapter::create_effect_rule);
    tsrs_lint::main();
}

/// Rust-only: without the `cli` feature (see Cargo.toml) there is no linter to run.
#[cfg(not(feature = "cli"))]
fn main() {
    ets_etsoxlintrunner::init();
    eprintln!(
        "tsrs-lint: built without the `cli` feature (cargo build -p ets_tsrs_lint --features cli)"
    );
    std::process::exit(1);
}
