// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics discardedRunPromiseToRunFork:warning
import { Effect } from "effect"

const program = Effect.log("server started").pipe(
  Effect.andThen(Effect.fail(new Error("bind failed")))
)

void Effect.runPromise(program)
