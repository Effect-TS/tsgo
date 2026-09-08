// @effect-diagnostics *:off
// @effect-diagnostics preferSchemaUnion:warning
import { Schema } from "effect"

export const Circle = Schema.Struct({ kind: Schema.Literal("circle"), radius: Schema.Number })
export const Square = Schema.Struct({ kind: Schema.Literal("square"), side: Schema.Number })

export type Shape = typeof Circle.Type | typeof Square.Type
