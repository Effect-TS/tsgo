import { Context, Effect } from "effect"
interface Service { readonly value: number }
const Key = Context.Service<Service>("Service")
declare const acquire: Effect.Effect<Service>
declare const program: Effect.Effect<string, never, Service>

// MUST TRIGGER: the curried call must use acquire, remove the callback, and replace the whole application.
export const curried = (Effect.flatMap((p: Service) => Effect.provideService(program, Key, p)))(acquire)
