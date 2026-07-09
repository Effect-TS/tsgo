// @effect-diagnostics multipleCatchTag:warning
import { Data, Effect, pipe } from "effect"

class NotFound extends Data.TaggedError("NotFound")<{}> {}
class Unauthorized extends Data.TaggedError("Unauthorized")<{}> {}
class ValidationError extends Data.TaggedError("ValidationError")<{}> {}
class Retryable extends Data.TaggedError("Retryable")<{}> {}

declare const program: Effect.Effect<string, NotFound | Unauthorized | ValidationError>

// Should trigger: two consecutive catchTag transformations can collapse into catchTags
export const shouldReportTwo = pipe(
  program,
  Effect.catchTag("NotFound", () => Effect.succeed("missing")),
  Effect.catchTag("Unauthorized", () => Effect.succeed("denied"))
)

// Should trigger: three consecutive catchTag transformations can collapse into catchTags
export const shouldReportThree = pipe(
  program,
  Effect.catchTag("NotFound", () => Effect.succeed("missing")),
  Effect.catchTag("Unauthorized", () => Effect.succeed("denied")),
  Effect.catchTag("ValidationError", () => Effect.succeed("invalid"))
)

// Should not trigger: a non-catch transformation interrupts the chain
export const shouldNotReportInterrupted = pipe(
  program,
  Effect.catchTag("NotFound", () => Effect.succeed("missing")),
  Effect.map((value) => value.toUpperCase()),
  Effect.catchTag("Unauthorized", () => Effect.succeed("denied"))
)

// Should not trigger: catchTag with orElse parameter must be ignored
export const shouldNotReportOrElse = pipe(
  program,
  Effect.catchTag("NotFound", () => Effect.succeed("missing"), (error) => Effect.fail(error)),
  Effect.catchTag("Unauthorized", () => Effect.succeed("denied"))
)

// Should not trigger: a handled tag is rethrown as another tag that is later caught
export const shouldNotReportRethrow = pipe(
  program,
  Effect.catchTag("NotFound", () => Effect.fail(new Retryable())),
  Effect.catchTag("Retryable", () => Effect.succeed("retried"))
)

// Should not trigger: a single catchTag is not enough
export const shouldNotReportSingle = pipe(
  program,
  Effect.catchTag("NotFound", () => Effect.succeed("missing"))
)

// Should trigger: consecutive catchTag calls in pipeable form too
export const shouldReportPipeable = program.pipe(
  Effect.catchTag("NotFound", () => Effect.succeed("missing")),
  Effect.catchTag("Unauthorized", () => Effect.succeed("denied"))
)
