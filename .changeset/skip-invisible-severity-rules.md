---
"@effect/tsgo": patch
---

Skip diagnostic rules below the minimum visible severity before executing them.

In `tsc` CLI mode without `includeSuggestionsInTsc`, suggestion- and message-severity diagnostics are dropped from the output after rules run. The rule runner now receives the minimum severity the caller can surface and skips such rules up front, avoiding their type-checker queries entirely. A rule below the threshold still runs when any directive in the file references it (for example `// @effect-diagnostics ruleName:error` or a wildcard), since directives can raise its severity and must be tracked for `unusedDirective` reporting. Emitted diagnostics are unchanged; on a large Effect monorepo build this removes roughly 1–2s of check time.
