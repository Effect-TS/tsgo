---
"@effect/tsgo": minor
---

Add the `emptySchemaStruct` diagnostic (correctness, default severity warning, Effect v3 and v4). `Schema.Struct({})` models the type `{}`, so decoding accepts any non-null value without validating it: `Schema.decodeUnknownSync(Schema.Struct({}))(42)` returns `42`.

```ts
import { Schema } from "effect"

export const Params = Schema.Struct({})
//                    ^^^^^^^^^^^^^^^^^ `Schema.Struct({})` models the type `{}`, so any non-null value passes decoding unvalidated. ...

// depending on intent
export const AnyObject = Schema.ObjectKeyword // Schema.Object in v3
export const OpenMap = Schema.Record(Schema.String, Schema.Unknown)
```

Only a literal `{}` with no index signatures is reported: `Schema.Struct(fields)`, spreads, v3 `Schema.Struct({}, Schema.Record(...))` and v4 `Schema.StructWithRest(Schema.Struct({}), [...])` are left alone. The general case has no quick fix, because the right replacement depends on intent.

As the `parameters` of `Tool.make` / `Tool.dynamic` (`effect/ai`), the message points to `Tool.EmptyParams`, and a quick fix replaces the call with it:

```ts
import { Schema } from "effect"
import { Tool } from "effect/ai"

export const GetTime = Tool.make("GetTime", {
  parameters: Schema.Struct({}), // -> Tool.EmptyParams
  success: Schema.String
})
```

The fix is not offered when the tool constructor is a named import (`import { make } from "effect/ai/Tool"`).
