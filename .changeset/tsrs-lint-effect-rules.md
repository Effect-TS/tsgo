---
"@effect/tsgo": minor
---

Experimental: run the Effect Oxlint rules on tsrs-lint, the Rust port of tsgolint (oxc-project/tsrs#624).

The Rust build adds a `tsrs-lint` executable (`crates/ets_tsrs_lint`) that registers every Effect rule with tsrs's headless linter, as the patched Go tsgolint does. Oxlint uses it in place of tsgolint through `OXLINT_TSGOLINT_PATH`:

```bash
cargo build -p ets_tsrs_lint --features cli
OXLINT_TSGOLINT_PATH="$PWD/target/debug/tsrs-lint" oxlint --type-aware
```

The rules report the same `effecttsgo/*` diagnostics, related-information labels and suggestions as the Go build. tsrs is pinned to the unreleased `feat/rust-tsgolint` branch until tsrs ships tsrs-lint.
