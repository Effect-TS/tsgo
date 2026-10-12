---
"@effect/tsgo": patch
---

Run Effect diagnostics after TypeScript checks unused identifiers so plugin symbol queries cannot suppress TS6133. For example, with `noUnusedParameters` enabled, `export function unusedPredicate(value: unknown): value is number { return true; }` now reports the unused `value` parameter with the plugin enabled, just as it does with the plugin disabled.
