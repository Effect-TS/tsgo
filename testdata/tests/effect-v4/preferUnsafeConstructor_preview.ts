// @effect-diagnostics *:off
// @effect-diagnostics preferUnsafeConstructor:warning
import { Effect, Scope } from "effect"

export const scope = Effect.runSync(Scope.make())
