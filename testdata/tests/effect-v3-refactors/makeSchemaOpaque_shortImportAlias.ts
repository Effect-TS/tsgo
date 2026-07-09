// refactor: 5:14-5:22
// @effect-v3
import { Schema as S } from "effect"

export const MyStruct = S.Struct({
  id: S.Number,
  name: S.String
})
