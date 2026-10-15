---
"@effect/tsgo": patch
---

Stop `experimentalApiUsage` and `unstableApiUsage` from causing TS2589 and follow-on type errors in generic calls such as `Match.when({ kind: "A0" }, ...)` over a wide union.

When a generic call infers a property back to the object literal, the rules look up the type the literal would get without its own inferences. They used to resolve the call again with inference blocked, which could instantiate types the original check never built. They now replay the checker's type-argument inference on the signature it already resolved and leave out the literal's argument, so return type inferences, inferences from the other arguments, defaults, and constraints apply as before. When that type has no matching property, the rules use the type parameters' constraints.

These cases are now reported too:

- Constraint properties inside inferred generator functions.
- Array and rest-parameter constraints, for example `generic([{ test: 42 }])` when `T extends Reportable[]` marks `test` as experimental.
- Literals inside a call that is itself an argument of a generic call, for example `generic(id({ test: 42 }))`.
