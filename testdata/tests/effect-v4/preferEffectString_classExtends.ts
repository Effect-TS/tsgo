// @effect-diagnostics *:off
// @effect-diagnostics preferEffectString:warning
import { Context, Effect } from "effect"

declare const value: string

export class Upper extends Context.Service<Upper, { readonly upper: string }>()("Upper", {
  make: Effect.gen(function*() {
    return { upper: value.toUpperCase() }
  })
}) {}

const base = <T>(make: () => T) => class { static make = make }

export class InExtends extends base(() => value.trim()) {
  readonly inBody = value.toLowerCase()
}
