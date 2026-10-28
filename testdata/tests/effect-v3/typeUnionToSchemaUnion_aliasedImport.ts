// @effect-v3
// @effect-diagnostics typeUnionToSchemaUnion:warning
import { Schema as S } from "effect"

export const Circle = S.Struct({ kind: S.Literal("circle"), radius: S.Number })
export const Square = S.Struct({ kind: S.Literal("square"), side: S.Number })

// MUST TRIGGER - the fix reuses the in-scope `Schema as S` named import
// (`export const Shape = S.Union(Circle, Square)`) and adds no new import
export type Shape = typeof Circle.Type | typeof Square.Type

// MUST TRIGGER - members accessed through the aliased namespace; the fix reuses `S`
type Primitive = typeof S.String.Encoded | typeof S.Boolean.Encoded

// MUST TRIGGER - `Schema.Schema.Encoded` reached through the aliased import; the fix
// uses `S.Union(Circle, Square)`
type HelperShapeEncoded = S.Schema.Encoded<typeof Circle> | S.Schema.Encoded<typeof Square>

// MUST NOT TRIGGER - mixed `.Type` and `.Encoded`
type Mixed = typeof S.String.Type | typeof S.Boolean.Encoded
