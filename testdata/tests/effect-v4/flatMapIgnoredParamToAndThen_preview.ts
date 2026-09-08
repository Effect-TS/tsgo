import { Effect } from "effect"

const acquireLock: Effect.Effect<void> = Effect.log("lock acquired")
const runJob: Effect.Effect<number> = Effect.succeed(42)

export const program = acquireLock.pipe(
  Effect.flatMap(() => runJob)
)
