---
"@effect/tsgo": minor
---

Warn when `diagnosticSeverity` contains an unknown Effect rule name, including in overrides and inherited configurations. For example, `"floatingEfect": "error"` now reports `effect(unknownRuleName)` instead of being silently ignored.

The check uses the fully merged configuration. Set `"unknownRuleName": "off"` to disable it or `"unknownRuleName": "error"` to raise its severity. Local keys are underlined in tsconfig; inherited keys without local syntax produce a diagnostic without a source location.
