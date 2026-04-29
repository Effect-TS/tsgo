---
"@effect/tsgo": patch
---

Fix `@effect/language-service` activation when the plugin is inherited through multiple `tsconfig` `extends` hops.

Effect diagnostics now continue to work for config chains like `tsconfig.json -> worker.json -> base.json` without duplicating the plugin stanza in intermediate configs.
