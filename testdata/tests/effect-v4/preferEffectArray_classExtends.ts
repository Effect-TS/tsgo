// @effect-diagnostics *:off
// @effect-diagnostics preferEffectArray:warning
import { Context, Effect } from "effect"

const xs: ReadonlyArray<number> = [1, 2, 3]

export class Doubled extends Context.Service<Doubled, { readonly all: ReadonlyArray<number> }>()("Doubled", {
  make: Effect.gen(function*() {
    return { all: xs.map((n) => n * 2) }
  })
}) {}

const base = <T>(make: () => T) => class { static make = make }

export class InExtends extends base(() => xs.find((n) => n > 1)) {
  readonly inBody = xs.filter((n) => n > 1)
}

interface WithFind { readonly find: number }
export class Implements extends base(() => 1) implements WithFind {
  readonly find = 1
}
