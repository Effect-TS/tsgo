// @effect-diagnostics *:off
// @effect-diagnostics preferTypedSchemaDecoder:warning

import { pipe, Schema } from "effect"
import { decodeUnknownSync } from "effect/Schema"
import * as SchemaParser from "effect/SchemaParser"

const Person = Schema.Struct({
  name: Schema.String,
  coordinates: Schema.Tuple([Schema.Number, Schema.Number])
})

const person = {
  name: "Ada",
  coordinates: [1, 2] as [number, number]
}

// Typed variables and contextually typed literals are reported.
export const sync = Schema.decodeUnknownSync(Person)(person)
export const literal = Schema.decodeUnknownSync(Person)({ name: "Ada", coordinates: [1, 2] })
export const effect = Schema.decodeUnknownEffect(Person)(person)
export const exit = Schema.decodeUnknownExit(Person)(person)
export const option = Schema.decodeUnknownOption(Person)(person)
export const result = Schema.decodeUnknownResult(Person)(person)
export const promise = Schema.decodeUnknownPromise(Person)(person)

// SchemaParser APIs and piping forms are normalized through piping flows.
export const parser = SchemaParser.decodeUnknownSync(Person)(person)
export const piped = pipe(person, Schema.decodeUnknownSync(Person))
export const named = decodeUnknownSync(Person)(person)

// Application options do not prevent recognizing the decoder application.
export const withOptions = Schema.decodeUnknownSync(Person)({ name: "Ada", coordinates: [1, 2] }, { errors: "all" })

declare const unknownInput: unknown
declare const anyInput: any
declare const wrongInput: { readonly name: number }

// Unknown, any, incompatible, and unresolved generic inputs are valid uses.
Schema.decodeUnknownSync(Person)(unknownInput)
Schema.decodeUnknownSync(Person)(anyInput)
Schema.decodeUnknownSync(Person)(wrongInput)
Schema.decodeUnknownSync(Person)({ name: 1, coordinates: [1, 2] })
Schema.decodeUnknownSync(Person)({ name: "Ada" })
Schema.decodeUnknownSync(Person)({ name: "Ada", coordinates: [1] })

function decodeGeneric<T>(input: T) {
  return Schema.decodeUnknownSync(Person)(input)
}

function decodeNestedGeneric<T extends string>(input: { readonly name: T; readonly coordinates: [number, number] }) {
  return Schema.decodeUnknownSync(Person)(input)
}

decodeGeneric(person)
decodeNestedGeneric(person)
