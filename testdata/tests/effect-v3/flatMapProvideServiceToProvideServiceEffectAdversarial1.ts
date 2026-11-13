// @effect-v3
import { Context, Effect } from "effect"
interface Service { readonly value: number }
const Key = Context.GenericTag<Service>("Service")
declare const acquire: Effect.Effect<Service>
declare const program: Effect.Effect<string, never, Service>

// MUST TRIGGER: both union members acquire the service, and the fix preserves both error channels.
export function union(input: Effect.Effect<Service, "a"> | Effect.Effect<Service, "b">) {
  return input.pipe(Effect.flatMap((p) => Effect.provideService(program, Key, p)))
}
