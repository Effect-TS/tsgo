// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics scopedImmediateScopeToScopedWith:warning
import { Effect, Scope } from "effect"

declare const openConnection: (scope: Scope.Scope) => Effect.Effect<string>

export const program = Effect.gen(function*() {
  const scope = yield* Effect.scope
  const connection = yield* openConnection(scope)
  yield* Scope.addFinalizer(scope, Effect.log("closing"))
  return connection
}).pipe(Effect.scoped)
