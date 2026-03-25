---
"@effect/tsgo": patch
---

Fix `@effect-diagnostics *:off` handling so only `skip-file` disables an entire file, allowing later rule-specific preview directives to re-enable diagnostics as in the upstream Effect language service.
