// refactor: 4:14-4:22
import * as Schema from "effect/Schema"

export const MyStruct = Schema.Struct({
  id: Schema.Number,
  name: Schema.String
})
