// @effect-diagnostics *:off
// @effect-diagnostics catchChainToFirstSuccessOf:warning
import { Effect, pipe } from "effect"

declare const first: Effect.Effect<string, Error>
declare const second: Effect.Effect<number, TypeError>
declare const third: Effect.Effect<boolean, Error>
declare const anyError: Effect.Effect<string, any>

// Should report: all earlier errors are included in the final Error type.
export const pipeable = first.pipe(
  Effect.catch(() => second),
  Effect.catch(() => third)
)

// Should report in free pipe form and accept a single-return block.
export const freePipe = pipe(
  first,
  Effect.catch(() => second),
  Effect.catch(() => {
    return third
  })
)

// Should report for nested data-first calls.
export const dataFirst = Effect.catch(
  Effect.catch(first, () => second),
  () => third
)

// Should not report: a single catch is not a fallback chain.
export const single = first.pipe(Effect.catch(() => second))

// Should not report: the handler depends on the previous error.
export const usesError = first.pipe(
  Effect.catch((error) => Effect.fail(error.message)),
  Effect.catch(() => third)
)

// Should not report: side logic makes the handler more than a lazy expression.
export const sideLogic = first.pipe(
  Effect.catch(() => {
    console.log("fallback")
    return second
  }),
  Effect.catch(() => third)
)

// Should not report: the firstSuccessOf error would be Error | string,
// while the catch chain retains only string.
export const widensError = first.pipe(
  Effect.catch(() => second),
  Effect.catch(() => Effect.fail("last" as const))
)

// Should not report: any passes assignability checks but would absorb the union.
export const anyErrorChannel = anyError.pipe(
  Effect.catch(() => second),
  Effect.catch(() => Effect.fail("last" as const))
)

// Should report: use the handler's annotated return type, not its body type.
export const annotatedReturn = first.pipe(
  Effect.catch((): Effect.Effect<number, Error> => second),
  Effect.catch(() => third)
)

// Should not report: another transformation interrupts the catch run.
export const interrupted = first.pipe(
  Effect.catch(() => second),
  Effect.map(String),
  Effect.catch(() => third)
)
