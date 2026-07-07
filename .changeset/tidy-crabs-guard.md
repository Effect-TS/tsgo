---
"@effect/tsgo": patch
---

Fix checker panics on `import.defer(...)` calls and bindingless import clauses.

`import.defer` parses as a meta property, and the checker debug-asserts (panics) when asked for its symbol or type while it is used as an import-call callee. Rules resolving arbitrary call callees (e.g. `catchUnfailableEffect`, `globalFetch`, `globalTimers`) crashed tsc and the LSP on files containing:

```ts
import.defer("./module")
```

Symbol resolution now goes through a guarded `TypeParser.GetSymbolAtLocation` wrapper that skips meta properties, and all rule/refactor/LSP call sites were audited to use it. `TypeParser.GetTypeAtLocation` gained the same meta-property guard, plus a guard for import clauses without a default binding (`import { A } from "x"`), which previously hit a nil-symbol panic that was silently recovered.

Also adds rule sweep stress tests that run the every-node diagnostics (`anyUnknownInErrorContext`, `effectInFailure`) over the typescript-go compiler test corpus, the effect-v4 fixtures, and effect's own package sources under a watchdog, failing on panics or non-termination.
