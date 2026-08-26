// @effect-diagnostics *:off
// @effect-diagnostics mapSomeToAsSome:warning
import { Effect, Option } from "effect"

const numberEffect = Effect.succeed(1)

export const pipeable = numberEffect.pipe(
  Effect.map(Option.some)
)

export const etaExpanded = numberEffect.pipe(
  Effect.map((value) => Option.some(value))
)

export const dataFirst = Effect.map(numberEffect, Option.some)

