// @filename: forms.ts
// @effect-diagnostics *:off
// @effect-diagnostics mapConstToAs:warning
import { Effect } from "effect"

enum Phase { Done = "done" }

declare const source: Effect.Effect<number>

// MUST TRIGGER: data-first form -> Effect.as(source, "saved")
export const dataFirst = Effect.map(source, () => "saved")

// MUST TRIGGER: data-last form applied directly -> Effect.as(-1)(source)
export const dataLastApplied = Effect.map(() => -1)(source)

// MUST TRIGGER: block body returning an enum member inside a longer pipe -> Effect.as(Phase.Done)
export const blockReturnInPipe = source.pipe(
  Effect.tap(() => Effect.void),
  Effect.map((_) => {
    return Phase.Done
  }),
  Effect.map((phase) => phase.length)
)

// @filename: aliasFallback.ts
// @effect-diagnostics *:off
// @effect-diagnostics mapConstToAs:warning
import * as Fx from "effect/Effect"

declare const source: Fx.Effect<number>
const mapAlias = Fx.map

// MUST TRIGGER: an aliased map; the fix reaches the Effect module through its namespace import
// -> Fx.as("saved")
export const value = source.pipe(mapAlias(() => "saved"))
