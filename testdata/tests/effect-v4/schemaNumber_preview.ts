// @effect-diagnostics *:off
// @effect-diagnostics schemaNumber:warning

import { Schema } from "effect"

export const User = Schema.Struct({
  age: Schema.Number,
  score: Schema.NumberFromString
})
