// refactor: 4:14-4:22
import { Schema } from "effect"

export const MyStruct = Schema.Struct({
  id: Schema.Number,
  name: Schema.String
})
