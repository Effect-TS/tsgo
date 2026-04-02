---
"@effect/tsgo": minor
---

Add `cryptoRandomUUID` and `cryptoRandomUUIDInEffect` diagnostics for Effect v4 to warn on `crypto.randomUUID()` usage and prefer the Effect `Random` module.

This ports the upstream language-service change into the Go implementation and adds matching v4 fixtures, baselines, metadata, and schema entries.
