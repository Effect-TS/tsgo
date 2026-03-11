// refactor: 4:14-4:22
import { Schema as S } from "effect"

export const MyStruct = S.Struct({
  id: S.Number,
  name: S.String
})
