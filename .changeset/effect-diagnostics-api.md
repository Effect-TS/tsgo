---
"@effect/tsgo": minor
---

Add a native `getEffectDiagnostics` API entrypoint that runs Effect diagnostics for a specific source file with explicit rule selection and Effect options.

This also exposes a shared internal rule runner so the checker hook and native API use the same directive, severity, and override behavior.
