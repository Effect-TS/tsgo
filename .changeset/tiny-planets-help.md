---
"@effect/tsgo": minor
---

Add `processEnv` and `processEnvInEffect` diagnostics to warn on `process.env` reads and recommend using Effect `Config` instead.

This ports the upstream language-service change into the Go implementation and adds matching v3/v4 fixtures, baselines, metadata, and schema entries.
