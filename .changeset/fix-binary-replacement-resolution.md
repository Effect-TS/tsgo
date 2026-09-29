---
"@effect/tsgo": patch
---

Fix `getExePath` rejecting the schema version shipped by platform packages.

Discover installed Oxlint musl bindings without rejecting them before component selection, so TypeScript-only commands work when Oxlint is installed on Alpine Linux. Report unsupported Oxlint replacements during patch preparation instead, allowing `--skip-missing` and restoring original binaries with `unpatch`.
