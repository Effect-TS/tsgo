// @effect-v3
// @effect-diagnostics forLoopPushToForEach:warning
import { Effect } from "effect"

declare const items: ReadonlyArray<number>
declare const processItem: (item: number) => Effect.Effect<string>

export const program = Effect.gen(function*() {
  const results = []
  for (const item of items) {
    results.push(yield* processItem(item))
  }
  return results
})
