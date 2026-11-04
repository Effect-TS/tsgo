// @effect-diagnostics *:off
// @effect-diagnostics promiseInEffectSuccess:warning
import { Context, Effect } from "effect"

declare const promiseValue: Promise<number>

export class Pending extends Context.Service<Pending, { readonly run: Effect.Effect<Promise<number>> }>()("Pending", {
  make: Effect.gen(function*() {
    return { run: Effect.succeed(promiseValue) }
  })
}) {}

const base = <T>(make: () => T) => class { static make = make }

export class InExtends extends base(() => Effect.succeed(promiseValue)) {
  readonly inBody = Effect.succeed(promiseValue)
}
