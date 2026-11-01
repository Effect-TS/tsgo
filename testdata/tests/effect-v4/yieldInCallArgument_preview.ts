// @effect-diagnostics *:off
// @effect-diagnostics yieldInCallArgument:warning
import { Effect } from "effect"

declare const load: Effect.Effect<number>
declare function transform(n: number): string

export const preview = Effect.gen(function*() {
  const result = transform(yield* load)
  return result
})
