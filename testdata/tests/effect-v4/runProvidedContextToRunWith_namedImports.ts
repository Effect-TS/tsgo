import type { Effect } from "effect"
import { Context } from "effect"
import { provideContext, runFork, runForkWith, runPromise } from "effect/Effect"

// Only named imports and a type-only Effect import: the fix must never introduce
// an `Effect.` reference that has no value binding.

declare const program: Effect.Effect<string, never, "Config">
declare const services: Context.Context<"Config">

// MUST TRIGGER - no fix (report only): no value binding in scope reaches
// Effect.runPromiseWith (the Effect import is type-only)
export const noValueReference = runPromise(provideContext(program, services))

// MUST TRIGGER - the fix uses the runForkWith named import that is in scope
// fix: runForkWith(services)(program)
export const namedReplacementInScope = runFork(provideContext(program, services))

// MUST NOT TRIGGER - already the *With form through a named import
export const alreadyNamedWith = runForkWith(services)(program)
