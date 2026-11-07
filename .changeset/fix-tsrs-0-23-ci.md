---
"@effect/tsgo": patch
---

Finish the tsrs 0.23.0 bump: refresh `Cargo.lock` for the new tsrs crate versions, and update the release parity check in `repoctl tsrs check` to read tsrs's new matrix-based `build-windows` job (same target, runner and build flags, now written as `${{ matrix.target }}`). Rework the Oxlint `002-effect-tsgolint-path.patch` so it applies to Oxlint 1.92.0 as well as 1.88–1.91: `EFFECT_TSGO_TSGOLINT_PATH` is now resolved by a wrapper around upstream's `try_find_tsgolint_executable` instead of an edit inside it (same behavior).
