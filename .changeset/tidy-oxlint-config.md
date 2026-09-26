---
"@effect/tsgo": patch
---

Preserve Effect plugin options in the Oxlint integration when `tsconfig.json` uses `extends`, whether the options are inherited or declared locally. For example, `effectFn: ["span", "inferred-span", "suggested-span"]` now enables the same `effect-fn-opportunity` reports as an inline configuration without `extends`.
