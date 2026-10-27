// refactor: 4:14-4:22
import * as X from "effect"

export const MyStruct = X.Schema.Struct({
  id: X.Schema.Number,
  name: X.Schema.String
})
