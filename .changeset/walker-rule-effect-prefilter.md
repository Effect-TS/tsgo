---
"@effect/tsgo": patch
---

Skip flow-analysis type queries for references that conclusively cannot be an Effect.

The `effectInFailure` and `promiseInEffectSuccess` rules walk every node of a file and query its flow type just to test whether it is a strict Effect type. The new `TypeParser.NodeCouldBeStrictEffect` prefilter inspects the referenced symbol's declared type first — flow narrowing can only refine the declared type, so a declared type that conclusively contains no possibly-Effect constituent (primitives, plain objects with a different type name, unions thereof) can never produce a strict Effect flow type, and the expensive query is skipped. The predicate is conservative: `any`/`unknown`, type parameters, conditionals, symbol-less types, and deep unions always fall through to the full query. Emitted diagnostics are unchanged; on a large Effect monorepo build this removes ~10% of build wall time (~2.7s of ~26.8s).
