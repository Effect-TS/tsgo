// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics runProvidedContextToRunWith:suggestion
import type { Effect } from "effect"
import { Context } from "effect"
import { provide, runFork, runPromise } from "effect/Effect"

// Effect v3 has no run*With runners, so nothing here may trigger.

class Config extends Context.Tag("Config")<Config, { readonly apiUrl: string }>() {}

declare const program: Effect.Effect<string, never, Config>
declare const services: Context.Context<Config>
declare const loadServices: () => Context.Context<Config>

// MUST NOT TRIGGER - named imports with only a type-only Effect import
export const namedOnly = runPromise(provide(program, services))

// MUST NOT TRIGGER - a computed context handed to a runner
export const computedContext = runFork(provide(program, loadServices()))
