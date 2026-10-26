---
"@effect/tsgo": patch
---

Fix the tsrs build reusing stale Effect diagnostics in incremental builds after the plugin options change. The build info now records the Effect options, as the Go build does, and a change to them drops the cached semantic diagnostics. For example, with `"incremental": true` and an existing `.tsbuildinfo`, switching `"diagnosticSeverity": { "floatingEffect": "off" }` to `"error"` now reports `floatingEffect` without `--force`, and switching it back stops reporting it.
