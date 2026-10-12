---
"@effect/tsgo": minor
---

Ship the Oxlint integration in a separate `@effect/tsgo-oxlint` package. The Effect builds of the Oxlint binding and `tsgolint` moved from the `@effect/tsgo-<platform>` packages to new `@effect/tsgo-oxlint-<platform>` packages, which `@effect/tsgo-oxlint` installs for the current platform. Projects that only use the TypeScript integration no longer download them, and `@effect/tsgo-darwin-arm64` is 98 MB smaller unpacked.

Projects that use the Oxlint integration must add `@effect/tsgo-oxlint` at the same version as `@effect/tsgo`:

```sh
pnpm add --save-dev @effect/tsgo-oxlint
```

`effect-tsgo patch --oxlint` loads the Oxlint builds from `@effect/tsgo-oxlint`, and fails with an install hint when the package is missing or its version differs from `@effect/tsgo`. `effect-tsgo setup` adds `@effect/tsgo-oxlint` when the Oxlint integration is selected and removes it when it is not.
