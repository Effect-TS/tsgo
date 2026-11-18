---
"@effect/tsgo": patch
---

`effect-tsgo patch --oxlint` and `effect-tsgo unpatch --oxlint` no longer require `oxlint-tsgolint`. They patch and restore the Oxlint binding and declarations, and `oxlint-tsgolint` only where it is installed, so the Oxlint integration now works in projects that do not install it, such as projects on Vite+ 1.7.0, which no longer installs `oxlint-tsgolint`.

```json
{
  "scripts": {
    "prepare": "effect-tsgo patch --oxlint"
  }
}
```
