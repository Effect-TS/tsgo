---
"@effect/tsgo": minor
---

Add a `"taggedClass"` target to `keyPatterns` for the `deterministicKeys` rule. It covers `Schema.TaggedClass`, `Data.TaggedClass` and `Schema.TaggedRequest`, and checks the tag. For `Schema.*` classes that is the first argument of the outer call, not the optional identifier. Every pattern works with the new target, and the `TaggedClass` and `TaggedRequest` completions use it to fill in the tag. Nothing changes unless the target is configured.

```jsonc
{
  "name": "@effect/language-service",
  "keyPatterns": [{ "target": "taggedClass", "pattern": "identifier" }]
}
```

```ts
// reports: the expected key is `Completed`
export class Completed extends Schema.TaggedClass<Completed>()("completed", {}) {}
```
