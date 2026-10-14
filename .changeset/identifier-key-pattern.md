---
"@effect/tsgo": minor
---

Add an `"identifier"` pattern to `keyPatterns` for the `deterministicKeys` rule. With it, the expected key is the class name alone.

```json
{
  "name": "@effect/language-service",
  "keyPatterns": [{ "target": "error", "pattern": "identifier" }]
}
```

```ts
// reports: the expected key is `KeybindingsConfigError`
export class KeybindingsConfigError extends Data.TaggedError("KeybindingsConfigParseError")<{}> {}
```
