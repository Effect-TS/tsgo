// @effect-diagnostics *:off
// @effect-diagnostics preferEffectRecord:warning
import { Context, Effect } from "effect"

const data = { a: 1, b: 2 }

export class Keys extends Context.Service<Keys, { readonly keys: ReadonlyArray<string> }>()("Keys", {
  make: Effect.gen(function*() {
    return { keys: Object.keys(data) }
  })
}) {}

const base = <T>(make: () => T) => class { static make = make }

export class InExtends extends base(() => Object.entries(data)) {
  readonly inBody = Object.values(data)
}
