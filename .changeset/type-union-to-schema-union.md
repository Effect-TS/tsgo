---
"@effect/tsgo": minor
---

Add the `typeUnionToSchemaUnion` diagnostic (style group, off by default, Effect v3 and v4). It reports a type alias that is a union of Schema `.Type` (or `.Encoded`) types with no runtime schema behind it, and offers a "Convert to Schema.Union" quickfix:

```ts
// before
export type Shape = typeof Circle.Type | typeof Square.Type
// after (v4; v3 emits Schema.Union(Circle, Square))
export const Shape = Schema.Union([Circle, Square])
export type Shape = typeof Shape.Type
```

`Schema.Schema.Type<typeof S>`, `Schema.Schema.Encoded<typeof S>` (v3) and `Schema.Codec.Encoded<typeof S>` (v4) members are also recognized. No fix is offered when a member schema is declared later in the file.
