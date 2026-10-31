// @effect-v3
// @filename: tsconfig.json
{"compilerOptions":{"strict":true,"plugins":[{"name":"@effect/language-service"}]}}

// @filename: forLoopPushToForEach_strict.ts
import { Effect } from "effect"

declare const items: ReadonlyArray<number>
declare const processItem: (item: number) => Effect.Effect<string>

// MUST TRIGGER: under strict the accumulator is an evolving array.
// fix: const results = yield* Effect.forEach(items, processItem)
export const program = Effect.gen(function*() {
  const results = []
  for (const item of items) {
    results.push(yield* processItem(item))
  }
  return results
})
