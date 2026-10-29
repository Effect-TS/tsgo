// @effect-diagnostics typeUnionToSchemaUnion:warning
import * as S from "effect/Schema"

export const Circle = S.Struct({ kind: S.Literal("circle"), radius: S.Number })
export const Square = S.Struct({ kind: S.Literal("square"), side: S.Number })

// MUST TRIGGER - the fix reuses the in-scope `S` namespace import
// (`export const Shape = S.Union([Circle, Square])`) and adds no new import
export type Shape = typeof Circle.Type | typeof Square.Type

// MUST TRIGGER - members accessed through the aliased namespace; the fix reuses `S`
type Primitive = typeof S.String.Encoded | typeof S.Boolean.Encoded

// MUST TRIGGER - `Schema.Schema.Type` reached through the aliased namespace; the fix
// uses `S.Union([Circle, Square])`
type HelperShape = S.Schema.Type<typeof Circle> | S.Schema.Type<typeof Square>

// MUST TRIGGER - `Schema.Codec.Encoded` reached through the aliased namespace; the fix
// uses `S.Union([Circle, Square])` and the alias becomes `typeof HelperShapeEncoded.Encoded`
type HelperShapeEncoded = S.Codec.Encoded<typeof Circle> | S.Codec.Encoded<typeof Square>

// MUST NOT TRIGGER - mixed `.Type` and `.Encoded`
type Mixed = typeof S.String.Type | typeof S.Boolean.Encoded
