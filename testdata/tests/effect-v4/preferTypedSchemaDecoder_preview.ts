// @effect-diagnostics *:off
// @effect-diagnostics preferTypedSchemaDecoder:warning

import { Schema } from "effect"

const Person = Schema.Struct({ name: Schema.String })

export const person = Schema.decodeUnknownSync(Person)({ name: "Ada" })
