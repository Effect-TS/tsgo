// @effect-diagnostics *:off
// @effect-diagnostics catchChainToFirstSuccessOf:warning
import { Effect } from "effect"

declare const primary: Effect.Effect<string, Error>
declare const secondary: Effect.Effect<string, Error>
declare const tertiary: Effect.Effect<string, Error>

export const preview = primary.pipe(
  Effect.catch(() => secondary),
  Effect.catch(() => tertiary)
)
