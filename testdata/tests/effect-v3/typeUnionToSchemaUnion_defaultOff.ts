// @effect-v3
import { Schema } from "effect"

export const Circle = Schema.Struct({ kind: Schema.Literal("circle"), radius: Schema.Number })
export const Square = Schema.Struct({ kind: Schema.Literal("square"), side: Schema.Number })

// MUST NOT TRIGGER - the rule is off by default and this file does not enable it
export type Shape = typeof Circle.Type | typeof Square.Type
