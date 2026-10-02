---
"@effect/tsgo": patch
---

Build packaged TypeScript compilers without embedded standard libraries so editor go-to-definition opens real library files. Ship matching `lib.*.d.ts` files beside each versioned compiler and the latest `lib/tsc` alias, keeping `diagnostics` and `get-exe-path` working when running the packaged executable directly.
