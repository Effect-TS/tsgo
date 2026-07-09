// @effect-diagnostics schemaNumber:warning

import { Schema } from "effect"
import * as SchemaModule from "effect/Schema"
import { Number as NumberSchema, NumberFromString } from "effect/Schema"

export const user = Schema.Struct({
  age: Schema.Number,
  score: Schema.NumberFromString,
  height: Schema.Finite,
  weight: Schema.FiniteFromString
})

export const product = SchemaModule.Struct({
  price: SchemaModule.Number,
  stock: SchemaModule.NumberFromString
})

export const direct = Schema.Struct({
  quantity: NumberSchema,
  amount: NumberFromString
})

// @effect-diagnostics-next-line schemaNumber:off
export const intentionallyNonFinite = Schema.Number
