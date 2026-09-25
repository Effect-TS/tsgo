---
"@effect/tsgo": patch
---

`effect-tsgo diagnostics` now reports files it skipped because their tsconfig does not enable the `@effect/language-service` plugin, instead of only printing `Checked 0 files out of N files.` The JSON summary gains a matching `filesWithoutPlugin` count.

```
Checked 0 files out of 8 files.
Skipped 8 files because their tsconfig does not enable the @effect/language-service plugin.
0 errors, 0 warnings and 0 messages.
```
