---
"@effect/tsgo": patch
---

Fix `supportedEffect` metadata for two rules whose declaration disagreed with their actual behavior:

- `genericEffectServices` declared `["v3", "v4"]` but is gated to run only on Effect v3 projects. It now declares `["v3"]`, so v4 users no longer enable a rule that can never fire.
- `schemaSyncInEffect` declared `["v3"]` but fully supports Effect v4 (suggesting `decodeEffect`/`encodeEffect` variants). It now declares `["v3", "v4"]`.

`metadata.json` and the generated rule docs were regenerated accordingly. No diagnostic behavior changed.
