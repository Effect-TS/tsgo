---
"@effect/tsgo": patch
---

Use the configured `keyPatterns` for `Data.TaggedError` and `Schema.TaggedError` completions, so the inserted tag already matches the key expected by `deterministicKeys`.

For example, with `{ "target": "error", "pattern": "package-identifier" }` in `keyPatterns`, completing `class NotFound extends Data.` in `src/errors.ts` of package `@app/core` now inserts `Data.TaggedError("@app/core/errors/NotFound")<{}>{}` instead of `Data.TaggedError("NotFound")<{}>{}`. Without an `error` key pattern the completion still uses the class name.
