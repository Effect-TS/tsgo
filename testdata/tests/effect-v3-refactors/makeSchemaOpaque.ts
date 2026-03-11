// refactor: 5:14-5:22
// @effect-v3
import * as Schema from "effect/Schema"

export const MyStruct = Schema.Struct({
  id: Schema.Number,
  name: Schema.String
})
