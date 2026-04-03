---
"@effect/tsgo": minor
---

Add `asyncFunction` and `newPromise` diagnostics to warn on `async` functions and manual `new Promise(...)` construction in favor of Effect-native async patterns.

This ports the upstream language-service change into the Go implementation and adds matching v3/v4 fixtures, baselines, metadata, and README updates.
