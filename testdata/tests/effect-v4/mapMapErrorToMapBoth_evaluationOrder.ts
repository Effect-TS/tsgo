// @effect-diagnostics *:off
// @effect-diagnostics mapMapErrorToMapBoth:warning
import { Effect } from "effect"

let multiplier = 1
const makeErrorMapper = () => {
  multiplier = 2
  return (message: string) => message.length
}
const makeSuccessMapper = () => {
  multiplier = 2
  return (n: number) => n * multiplier
}
let errorMapper = (message: string) => message.length

// MUST TRIGGER (no quickfix): the inner Effect.mapEager runs its callback on the resolved
// effect before makeErrorMapper() is evaluated (result 3); Effect.mapBothEager would
// evaluate makeErrorMapper() first (result 6)
export const eagerNestedCall = Effect.mapErrorEager(
  Effect.mapEager(Effect.succeed(3), (n) => n * multiplier),
  makeErrorMapper()
)

// MUST TRIGGER (no quickfix): reverse order, the inner Effect.mapErrorEager runs first on a failure
export const eagerNestedCallReverse = Effect.mapEager(
  Effect.mapErrorEager(Effect.fail("boom"), (message) => message.length * multiplier),
  makeSuccessMapper()
)

// MUST TRIGGER (no quickfix): a mutable binding read after the inner step ran
export const eagerNestedIdentifier = Effect.mapErrorEager(
  Effect.mapEager(Effect.succeed(3), (n) => {
    errorMapper = (message) => message.length * 2
    return n
  }),
  errorMapper
)

// MUST TRIGGER: a function literal runs no code when evaluated, so the quickfix is offered
export const eagerNestedFunctionLiteral = Effect.mapErrorEager(
  Effect.mapEager(Effect.succeed(3), (n) => n * multiplier),
  (function(message: string) {
    return message.length
  })
)

// MUST TRIGGER: eager pipe; every pipe argument is evaluated before any step runs, so the
// quickfix is offered even for a call expression
export const eagerPipeCall = Effect.succeed(3).pipe(
  Effect.mapEager((n) => n * multiplier),
  Effect.mapErrorEager(makeErrorMapper())
)

// MUST TRIGGER: non-eager nested data-first; Effect.map does not run its callback
// when called, so the quickfix is offered
export const lazyNestedCall = Effect.mapError(
  Effect.map(Effect.succeed(3), (n) => n * multiplier),
  makeErrorMapper()
)
