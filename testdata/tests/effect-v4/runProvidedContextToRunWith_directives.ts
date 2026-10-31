// @effect-diagnostics runProvidedContextToRunWith:warning
import { Context, Effect } from "effect"

declare const program: Effect.Effect<string, never, "Config">
declare const services: Context.Context<"Config">

// MUST TRIGGER - raised to a warning by the file directive
export const raised = Effect.runPromise(Effect.provideContext(program, services))

// MUST NOT TRIGGER - disabled for the next line only
// @effect-diagnostics-next-line runProvidedContextToRunWith:off
export const disabledNextLine = Effect.runPromise(Effect.provideContext(program, services))

// MUST TRIGGER - the next-line directive does not reach this run
export const afterNextLine = Effect.runSync(Effect.provideContext(program, services))

// @effect-diagnostics runProvidedContextToRunWith:off

// MUST NOT TRIGGER - disabled for the rest of the file
export const disabled = Effect.runPromise(Effect.provideContext(program, services))
