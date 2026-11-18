---
"@effect/tsgo": patch
---

`patch --tsrs` and `unpatch --tsrs` now find the `tsrs` installed through `vite-plus` when it is not hoisted to the project root, for example with npm's `install-strategy=linked` or with pnpm, and `effect-tsgo tsrs` and `--tsrs-lint` pick that installed version when this release ships a build of it.
