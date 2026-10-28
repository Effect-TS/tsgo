---
"@effect/tsgo": minor
---

Add the `globalJson` and `globalJsonInEffect` diagnostics (Effect-native, off by default, Effect v3 and v4). Like `globalRandom` / `globalRandomInEffect`, they report calls to the global `JSON.parse` and `JSON.stringify` outside and inside Effect code (`Effect.gen`, `Effect.fn`, and thunks such as `Effect.sync`, `Effect.try` and `Effect.tryPromise`), with the Schema replacement for the Effect version in use. Aliases of `JSON` are reported; a shadowed local `JSON` is not.

```ts
// globalJson: JSON.parse throws instead of returning a failure
const parseUserResult = (text: string) => Schema.decodeUnknownResult(User)(JSON.parse(text))
//                                                                         ^^^^^^^^^^^^^^^^
// This code uses `JSON.parse`, which throws instead of failing with a typed error. Use `Schema.fromJsonString(Schema.Unknown)` for unknown shapes, `Schema.fromJsonString(schema)` for known ones, or `Schema.toCodecJson(schema)` when working with JSON values instead of strings.

// globalJsonInEffect
const program = Effect.gen(function*() {
  const json = JSON.parse(text)
  //           ^^^^^^^^^^^^^^^ This Effect code uses `JSON.parse`, ...
})
```

`preferSchemaOverJson` is deprecated in favor of `globalJsonInEffect` and `globalJson`. It keeps working with its own message and reports what `globalJsonInEffect` reports, and the `effect-native` preset no longer includes it. Configuring a deprecated rule in `diagnosticSeverity` (or in an override) now reports the new `deprecatedRuleName` diagnostic on the tsconfig key, once per key:

```
Effect diagnostic rule `preferSchemaOverJson` is deprecated; use `globalJsonInEffect` or `globalJson` instead. effect(deprecatedRuleName)
```

Set `"deprecatedRuleName": "off"` to silence it or `"deprecatedRuleName": "error"` to raise its severity.
