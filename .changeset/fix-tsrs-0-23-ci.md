---
"@effect/tsgo": patch
---

Finish the tsrs 0.23.0 bump: refresh `Cargo.lock` for the new tsrs crate versions, and update the release parity check in `repoctl tsrs check` to read tsrs's new matrix-based `build-windows` job (same target, runner and build flags, now written as `${{ matrix.target }}`).
