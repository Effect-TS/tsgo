// @effect-diagnostics schemaNumber:warning

import { Schema } from "effect"
import * as SchemaModule from "effect/Schema"
import { check as refine, isFinite, isInt as integer, Number as NumberSchema, NumberFromString } from "effect/Schema"

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

export const refinements = Schema.Struct({
  finite: Schema.Number.check(Schema.isFinite()),
  integer: Schema.Number.check(Schema.isInt()),
  importedFinite: Schema.Number.check(isFinite()),
  importedInteger: Schema.Number.check(integer()),
  annotated: Schema.Number.annotate({ description: "a" }).check(Schema.isFinite()),
  annotatedPredicate: Schema.Number.check(Schema.isFinite({ description: "a" })),
  pipedFinite: Schema.Number.pipe(Schema.check(Schema.isFinite())),
  pipedInteger: Schema.Number.pipe(Schema.check(Schema.isInt())),
  pipedImported: Schema.Number.pipe(refine(integer())),
  pipedAnnotated: Schema.Number.pipe(
    Schema.annotate({ description: "a" }),
    Schema.check(Schema.isFinite())
  )
})
// @effect-diagnostics-next-line schemaNumber:off
export const intentionallyNonFinite = Schema.Number
