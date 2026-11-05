// @effect-diagnostics *:off
// @effect-diagnostics anyUnknownInErrorContext:warning
import { Context, Effect } from "effect"

export class Failing extends Context.Service<Failing, { readonly run: () => void }>()("Failing", {
  make: Effect.gen(function*() {
    yield* Effect.fail<unknown>(42)
    return { run: () => {} }
  })
}) {}

const base = <T>(make: () => T) => class { static make = make }

export class InExtends extends base(() => Effect.fail<any>(42)) {
  readonly inBody = Effect.fail<any>(42)
}
