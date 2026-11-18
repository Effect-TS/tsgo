---
"@effect/tsgo": patch
---

`patch --tsrs` and `unpatch --tsrs` now find the `tsrs` installed through `vite-plus` when it is not hoisted to the project root, as with npm's `install-strategy=linked` or pnpm. `effect-tsgo tsrs` and `--tsrs-lint` now pick that installed version when this release ships a build of it.
