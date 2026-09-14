// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics schemaSync:warning
import * as Schema from "effect/Schema"

const Person = Schema.Struct({ name: Schema.String })

export const person = Schema.decodeUnknownSync(Person)({ name: "Ada" })
