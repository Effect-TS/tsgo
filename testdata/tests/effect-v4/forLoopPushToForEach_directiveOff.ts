// @effect-v4
// @effect-diagnostics forLoopPushToForEach:off
import { Effect } from "effect"

declare const items: ReadonlyArray<number>
declare const processItem: (item: number) => Effect.Effect<string>

// MUST NOT TRIGGER: the file directive turns the rule off.
export const program = Effect.gen(function*() {
  const results = []
  for (const item of items) {
    results.push(yield* processItem(item))
  }
  return results
})
