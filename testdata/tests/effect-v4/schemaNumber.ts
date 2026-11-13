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
  annotateMethodFinite: Schema.Number.check(Schema.isFinite().annotate({ expected: "a finite number" })),
  annotateMethodInt: Schema.Number.check(Schema.isInt().annotate({ expected: "an integer" })),
  annotateMethodChained: Schema.Number.check(integer().annotate({ title: "a" }).annotate({ description: "b" })),
  annotateMethodNumberFromString: Schema.NumberFromString.check(Schema.isInt().annotate({ expected: "an integer" })),
  pipedAnnotateMethod: Schema.Number.pipe(Schema.check(Schema.isInt().annotate({ expected: "an integer" }))),
  abortMethodInt: Schema.Number.check(Schema.isInt().abort()),
  abortMethodFinite: Schema.Number.check(Schema.isFinite().abort()),
  abortMethodImported: Schema.Number.check(integer().abort()),
  abortMethodWithLaterCheck: Schema.Number.check(Schema.isInt().abort(), Schema.isBetween({ minimum: 1, maximum: 24 })),
  abortMethodNumberFromString: Schema.NumberFromString.check(Schema.isFinite().abort()),
  abortMethodParenthesized: Schema.Number.check((Schema.isInt()).abort()),
  andMethodInt: Schema.Number.check(Schema.isInt().and(Schema.isGreaterThan(0))),
  andMethodFinite: Schema.Number.check(Schema.isFinite().and(Schema.isLessThan(10), { expected: "a small number" })),
  andMethodChained: Schema.Number.check(Schema.isInt().and(Schema.isGreaterThan(0)).and(Schema.isLessThan(10))),
  mixedWrappers: Schema.Number.check(
    Schema.isInt().annotate({ expected: "an integer" }).abort().and(Schema.isGreaterThan(0)).annotate({ title: "a" })
  ),
  andArgumentInt: Schema.Number.check(Schema.isGreaterThan(0).and(Schema.isInt())),
  andArgumentAnnotatedAbort: Schema.Number.check(
    Schema.isGreaterThan(0).and(Schema.isFinite().annotate({ expected: "a finite number" }).abort())
  ),
  andArgumentNested: Schema.Number.check(
    Schema.isGreaterThan(0).and(Schema.isLessThan(10).and(integer())).annotate({ title: "a" })
  ),
  pipedAbortMethod: Schema.Number.pipe(Schema.check(Schema.isInt().abort())),
  pipedAndMethod: Schema.Number.pipe(refine(isFinite().and(Schema.isGreaterThan(0)))),
  pipedAndArgument: Schema.Number.pipe(Schema.check(Schema.isGreaterThan(0).and(Schema.isInt()))),
  pipedFinite: Schema.Number.pipe(Schema.check(Schema.isFinite())),
  pipedInteger: Schema.Number.pipe(Schema.check(Schema.isInt())),
  pipedImported: Schema.Number.pipe(refine(integer())),
  pipedAnnotated: Schema.Number.pipe(
    Schema.annotate({ description: "a" }),
    Schema.check(Schema.isFinite())
  )
})
export const stillReported = Schema.Struct({
  abortNonFinitePredicate: Schema.Number.check(Schema.isGreaterThan(0).abort()),
  andNonFiniteOperands: Schema.Number.check(Schema.isGreaterThan(0).and(Schema.isLessThan(10))),
  andNonFiniteOperandsAborted: Schema.Number.check(Schema.isGreaterThan(0).abort().and(Schema.isLessThan(10).abort())),
  pipedAbortNonFinitePredicate: Schema.Number.pipe(Schema.check(Schema.isGreaterThan(0).abort()))
})

// @effect-diagnostics-next-line schemaNumber:off
export const intentionallyNonFinite = Schema.Number
