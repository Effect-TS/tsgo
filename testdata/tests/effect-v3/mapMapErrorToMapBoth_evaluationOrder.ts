// @effect-diagnostics *:off
// @effect-diagnostics mapMapErrorToMapBoth:warning
import { Effect } from "effect"

let multiplier = 1
const makeErrorMapper = () => {
  multiplier = 2
  return (message: string) => message.length
}

// MUST TRIGGER: non-eager nested data-first; Effect.map does not run its callback
// when called, so evaluating makeErrorMapper() earlier is safe and the quickfix is offered
export const lazyNested = Effect.mapError(
  Effect.map(Effect.succeed(3), (n) => n * multiplier),
  makeErrorMapper()
)

// MUST TRIGGER: non-eager pipe; every pipe argument is evaluated before any step runs
export const lazyPipe = Effect.succeed(3).pipe(
  Effect.map((n) => n * multiplier),
  Effect.mapError(makeErrorMapper())
)
