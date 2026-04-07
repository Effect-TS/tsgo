---
"@effect/tsgo": patch
---

Fix Effect v4 service parsing for `effect@4.0.0-beta.43` and update the embedded v4 test packages to that version.

This keeps `ServiceMap.Service` detection working with the new `Identifier` / `Service` type shape while preserving the existing v3-only `Context.Tag` behavior.
