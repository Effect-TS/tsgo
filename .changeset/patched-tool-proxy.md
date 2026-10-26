---
"@effect/tsgo": minor
---

Add proxy commands that run the Effect-patched tsc, tsrs and Oxlint shipped with `@effect/tsgo` directly, without patching `node_modules`. This works with shared or read-only package stores such as the pnpm global virtual store, where `effect-tsgo patch` cannot rewrite the installed binaries. Every argument after the tool is passed through unchanged.

```sh
effect-tsgo tsc -p tsconfig.json         # the build of the installed TypeScript version, else the latest
effect-tsgo tsc@7.0.2 --lsp --stdio      # a specific supported TypeScript version
effect-tsgo tsc@next --build             # an upstream tag: latest, beta, next
effect-tsgo tsrs --noEmit                # the build of the installed tsrs version, else the latest
effect-tsgo oxlint --type-aware          # the installed oxlint CLI with the patched binding and tsgolint
```

The type augmentation that adds the `effecttsgo` plugin to Oxlint's config types is still applied only by `effect-tsgo patch`.
