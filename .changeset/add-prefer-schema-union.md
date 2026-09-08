---
"@effect/tsgo": minor
---

Add `preferSchemaUnion` diagnostic (`TS377130`) and code fix for Effect v4 to suggest composing Schema values with `Schema.Union` instead of unioning their extracted `Type` properties.

### Example

```ts
// Before
export const Circle = Schema.Struct({ kind: Schema.Literal("circle"), radius: Schema.Number })
export const Square = Schema.Struct({ kind: Schema.Literal("square"), side: Schema.Number })

export type Shape = typeof Circle.Type | typeof Square.Type

// After applying code fix
export const Circle = Schema.Struct({ kind: Schema.Literal("circle"), radius: Schema.Number })
export const Square = Schema.Struct({ kind: Schema.Literal("square"), side: Schema.Number })

export const Shape = Schema.Union([Circle, Square])
export type Shape = typeof Shape.Type
```
