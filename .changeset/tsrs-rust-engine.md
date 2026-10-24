---
"@effect/tsgo": minor
---

Add experimental support for [tsrs](https://www.npmjs.com/package/tsrs), the Rust port of the TypeScript 7 compiler. All Effect diagnostics, quick fixes, refactors and completions are ported to Rust and produce the same output as the Go version.

`effect-tsgo patch --tsrs` patches your installed `tsrs` package, replacing its binary with an Effect-patched build of the same tsrs version. TypeScript patching is unchanged.

```sh
npm install --save-dev tsrs@0.18.0-ts7.1.0-dev.20260929
npx effect-tsgo patch --tsrs        # patch TypeScript and tsrs
npx effect-tsgo patch --no-typescript --tsrs   # patch tsrs only
npx tsrs -p tsconfig.json
# src/index.ts(4,3): error TS377001: This Effect value is neither yielded nor used in an assignment. effect(floatingEffect)
npx effect-tsgo unpatch --tsrs      # restore tsrs's original binary
```

The patched tsrs binary is available on darwin-arm64, linux-x64 (glibc) and linux-arm64 (glibc). Other platforms, including Linux musl, are skipped with a message explaining why.
