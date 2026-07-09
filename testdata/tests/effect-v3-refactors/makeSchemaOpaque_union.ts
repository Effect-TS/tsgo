// refactor: 5:14-5:21
// @effect-v3
import * as Schema from "effect/Schema"

export const MyUnion = Schema.Union(
  Schema.Literal("A"),
  Schema.Literal("B")
)
