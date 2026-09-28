---
"@effect/tsgo": patch
---

Only discover the binaries for the requested integration components. `effect-tsgo get-exe-path`, `diagnostics`, and `patch --typescript --no-oxlint` no longer probe Oxlint, so they work on platforms the packaged Oxlint integration does not support, such as Linux musl (Alpine). `@effect/tsgo/lib/getExePath` now accepts the current `lib/upstream.json` schema version.
