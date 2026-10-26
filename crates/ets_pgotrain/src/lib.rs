//! Rust-only: no code. The crate exists for its dependencies (Cargo.toml): with the `train` feature, tsrs's
//! `tsrs-test` and `fourslash_server` binaries can be built from this workspace, with the crate units of the `tsrs`
//! binary (`crates/ets_tsrs`), for the release build's PGO and BOLT training (`repoctl build artifact --optimize`).
