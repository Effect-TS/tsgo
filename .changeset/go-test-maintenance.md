---
"@effect/tsgo": patch
---

Consolidate the Go test suite (about 14,000 fewer lines of Go test code) without changing any diagnostic behaviour:

- move scenario-style Go tests into `testdata/tests` fixtures (about 50 new fixtures), so their behaviour is pinned by baselines and also runs against the Rust port; for example the `rules` and `rulerunner` shape tests of `flatMapToTap`, `mapConstToAs` or `apiStabilityLeak` now live in fixtures such as `flatMapToTap_malformedCallbacks.ts`;
- remove tests duplicated across `internal/rules`, `internal/rulerunner` and `internal/effecttest`, and tautological tests;
- merge near-identical tests into table tests and share one "each quick fix still compiles" helper;
- remove narrating comments from test files;
- remove the native Array/Object/String method inventory docs;
- move the Rust port documentation (`PORTING.md`, `TSRS_PORT.md`, `TSRS_API.md`, `DIVERGENCES.md`) from `docs/` to `porting/`.
