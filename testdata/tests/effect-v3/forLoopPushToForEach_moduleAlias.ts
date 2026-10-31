// @effect-v3
import * as Fx from "effect/Effect"

declare const items: ReadonlyArray<number>
declare const processItem: (item: number) => Fx.Effect<string>

// MUST TRIGGER: the fix references forEach through the file's Effect module alias.
// fix: const results = yield* Fx.forEach(items, processItem)
export const moduleAlias = Fx.gen(function*() {
  const results = []
  for (const item of items) {
    results.push(yield* processItem(item))
  }
  return results
})
