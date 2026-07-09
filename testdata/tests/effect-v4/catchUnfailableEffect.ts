import { Effect, Data, pipe } from "effect"

class MyError extends Data.TaggedError("MyError")<{ message: string }> {}

declare const effectWithErrors: Effect.Effect<number, MyError>

// === Should trigger (E is never) ===

// .pipe() with catchTag on a never-failing effect
export const shouldReportPipeable = Effect.succeed(42).pipe(
  Effect.catchTag("MyError", () => Effect.void)
)

// pipe() function with catchTag
export const shouldReportPipe = pipe(
  Effect.succeed(42),
  Effect.catchTag("MyError", () => Effect.void)
)

// pipe() with catchTags
export const shouldReportCatchTags = pipe(
  Effect.succeed(42),
  Effect.catchTags({})
)

// pipe() with catchIf
export const shouldReportCatchIf = pipe(
  Effect.succeed(42),
  Effect.catchIf((e) => true, () => Effect.void)
)

// Effect.fn with trailing catch transformation
export const shouldReportFn = Effect.fn(function*() {
  return yield* Effect.succeed(42)
}, Effect.catchTag("MyError", () => Effect.void))

// Chain where intermediate steps preserve never-fail
export const shouldReportThird = pipe(
  Effect.succeed(42),
  Effect.map((n) => n + 1),
  Effect.catchTag("MyError", () => Effect.void)
)

// === Should NOT trigger (E is not never) ===

// Effect that can fail
export const shouldNotReport = effectWithErrors.pipe(
  Effect.catchTag("MyError", () => Effect.succeed(42))
)

// Chain where an intermediate step introduces failure
export const shouldNotReportChain = pipe(
  Effect.succeed(42),
  Effect.flatMap(() => Effect.fail(new MyError({ message: "oops" }))),
  Effect.catchTag("MyError", () => Effect.succeed(42))
)
