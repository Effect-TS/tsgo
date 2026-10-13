---
"@effect/tsgo": patch
---

Compute Effect diagnostics with less work. Rules and the type parser now test node kinds, shapes and argument counts before they query types, symbols or signatures, and they compute types only for steps that can still report:

- `IsNodeReferenceToModuleExport` answers symbols with no declaration in the requested module from a per-symbol bitmask instead of a per-symbol map.
- `DataFirstOrLastCall` checks the cached signature shape before it types call arguments, and caches its result per call.
- `missedPipeableOpportunity`, `catchDieToOrDie` and `preferTypedSchemaDecoder` type piping-flow steps only when a step matches.
- The `experimentalApiUsage` and `unstableApiUsage` rules reuse their own symbol lookup instead of repeating it.

With every rule enabled, `tsc -b` of the Effect monorepo drops from 33.9s to 32.1s (stock TypeScript-Go: 20.4s). The reported diagnostics are unchanged.
