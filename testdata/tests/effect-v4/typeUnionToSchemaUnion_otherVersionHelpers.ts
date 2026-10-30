// @effect-diagnostics typeUnionToSchemaUnion:warning
import { Schema } from "effect"

export const Circle = Schema.Struct({ kind: Schema.Literal("circle"), radius: Schema.Number })
export const Square = Schema.Struct({ kind: Schema.Literal("square"), side: Schema.Number })

// MUST NOT TRIGGER - `Schema.Schema.Encoded` is the v3 Encoded helper and does not exist in Effect v4
export type V3EncodedHelper = Schema.Schema.Encoded<typeof Circle> | typeof Square.Encoded

// MUST NOT TRIGGER - `Schema.Schema.Context` does not exist in Effect v4 and is not an accepted member form
export type HelperContext = Schema.Schema.Context<typeof Circle> | Schema.Schema.Context<typeof Square>
