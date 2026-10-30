// @effect-v4
import { Effect } from "effect"

declare const items: ReadonlyArray<number>
declare const processItem: (item: number) => Effect.Effect<string>

// MUST NOT TRIGGER: plain yield instead of yield*. Kept apart from the shapes
// fixture because a plain yield of an Effect does not type-check.
// MUST NOT TRIGGER: plain yield instead of yield*.
export const plainYield = Effect.gen(function*() {
  const out: Array<unknown> = []
  for (const item of items) out.push(yield processItem(item))
  return out
})
