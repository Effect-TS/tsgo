---
"@effect/tsgo": patch
---

Ship the latest TypeScript compiler once in each platform package. It was packaged twice, as `lib/tsc` and as `artifacts/typescript/<latest>/tsc`, each with its own `lib.*.d.ts` files. It now ships only as `lib/tsc`, which makes `@effect/tsgo-darwin-arm64` 29 MB smaller unpacked.

`get-exe-path`, `diagnostics`, `patch` and `getExePath()` resolve the latest TypeScript version to `lib/tsc` and other versions to `artifacts/typescript/<version>/tsc`. Tools that run `lib/tsc` directly keep working.
