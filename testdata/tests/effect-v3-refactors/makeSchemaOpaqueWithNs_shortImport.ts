// refactor: 5:14-5:22
// @effect-v3
import { Schema } from "effect"

export const MyStruct = Schema.Struct({
  id: Schema.Number,
  name: Schema.String
})
