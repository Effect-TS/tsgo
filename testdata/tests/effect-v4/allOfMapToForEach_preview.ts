// @effect-v4
// @effect-diagnostics allOfMapToForEach:warning
import { Effect } from "effect"

declare const values: ReadonlyArray<number>

export const program = Effect.all(
  values.map((value) => Effect.succeed(value + 1))
)
