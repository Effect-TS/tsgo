// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics runProvidedContextToRunWith:warning
import { Context, Effect } from "effect"

declare const program: Effect.Effect<string, never, "Config">
declare const services: Context.Context<"Config">

export const result = Effect.runPromise(Effect.provideContext(program, services))
