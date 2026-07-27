// @effect-diagnostics schemaLiteralNonFinite:error

import { Schema } from "effect"
import * as SchemaModule from "effect/Schema"

Schema.Literal(Infinity)
Schema.Literal(-Infinity)
Schema.Literal(Number.POSITIVE_INFINITY)
Schema.Literal(Number.NEGATIVE_INFINITY)
Schema.Literal(Number.NaN)
Schema.Literal(-Number.POSITIVE_INFINITY)
Schema.Literal(Number.POSITIVE_INFINITY - Number.POSITIVE_INFINITY)
Schema.Literal(Number["NaN"])
Schema.Literal(1e999)
Schema.Literal(1 / 0)
Schema.Literal(0 / 0)
Schema.Literals([0, 1, NaN])
SchemaModule.tag(NaN)

Schema.Literal(1)
Schema.Literal("Infinity")
Schema.Literals([0, 1, 2])

function localValues(Infinity: number, NaN: number, Number: { readonly NaN: number }) {
  Schema.Literal(Infinity)
  Schema.Literal(NaN)
  Schema.Literal(Number.NaN)
}

let reassigned = Infinity
reassigned = 1
Schema.Literal(reassigned)

const constantNaN = Number.NaN
const constantInfinity = Number.POSITIVE_INFINITY
Schema.Literal(constantNaN)
Schema.Literal(-constantInfinity)
