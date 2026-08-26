// @effect-diagnostics *:off
// @effect-diagnostics catchConditionalRefailToCatchIf:suggestion
import { Cause, Data, Effect } from "effect"

class NotFound extends Data.TaggedError("NotFound")<{}> {}
class DatabaseError extends Data.TaggedError("DatabaseError")<{}> {}

declare const program: Effect.Effect<string, NotFound | DatabaseError>
declare const otherError: NotFound | DatabaseError
declare const enabled: boolean

const isNotFound = (error: NotFound | DatabaseError): boolean => error._tag === "NotFound"

// Predicate call in a pipeable catch.
export const predicateTernary = program.pipe(
  Effect.catch((error) => isNotFound(error) ? Effect.succeed("guest") : Effect.fail(error))
)

// Re-fail in the true branch requires the inverse predicate.
export const invertedTernary = program.pipe(
  Effect.catch((error) => error instanceof DatabaseError ? Effect.fail(error) : Effect.succeed("guest"))
)

// A lone if/else with returning branches.
export const ifElse = program.pipe(
  Effect.catch(function(error) {
    if ("_tag" in error) {
      return Effect.succeed("guest")
    } else {
      return Effect.fail(error)
    }
  })
)

// A returning if followed by the fallback return.
export const ifThenReturn = program.pipe(
  Effect.catch((error) => {
    if (isNotFound(error)) return Effect.succeed("guest")
    return Effect.fail(error)
  })
)

// A block containing only a returned conditional is still a single ternary.
export const returnedTernary = program.pipe(
  Effect.catch((error) => {
    return isNotFound(error) ? Effect.succeed("guest") : Effect.fail(error)
  })
)

// Data-first calls are represented by the same piping-flow transformation.
export const dataFirst = Effect.catch(
  program,
  (error) => typeof error === "object" ? Effect.succeed("guest") : Effect.fail(error)
)

// Direct tag recovery maps to catchTag.
export const directTag = program.pipe(
  Effect.catch((error) => error._tag === "NotFound" ? Effect.succeed("guest") : Effect.fail(error))
)

// The tagged-dispatch parser also recognizes a one-case switch.
export const tagSwitch = program.pipe(
  Effect.catch((error) => {
    switch (error._tag) {
      case "NotFound":
        return Effect.succeed("guest")
      default:
        return Effect.fail(error)
    }
  })
)

declare const causeProgram: Effect.Effect<string, DatabaseError>

// catchCause must pair with failCause and maps to catchCauseIf.
export const cause = causeProgram.pipe(
  Effect.catchCause((cause) => Cause.hasFails(cause) ? Effect.succeed("guest") : Effect.failCause(cause))
)

// Should NOT trigger: the re-fail argument is mapped.
export const mappedRefail = program.pipe(
  Effect.catch((error) => isNotFound(error) ? Effect.succeed("guest") : Effect.fail(otherError))
)

// Should NOT trigger: the condition is unrelated to the handler parameter.
export const unrelatedCondition = program.pipe(
  Effect.catch((error) => enabled ? Effect.succeed("guest") : Effect.fail(error))
)

// Should NOT trigger: a three-way conditional does not map directly to catchIf.
export const multiBranch = program.pipe(
  Effect.catch((error) => isNotFound(error)
    ? Effect.succeed("guest")
    : error instanceof DatabaseError
      ? Effect.succeed("database")
      : Effect.fail(error))
)

// Should NOT trigger: catchTag is intentionally outside this rule.
export const alreadyCatchTag = program.pipe(
  Effect.catchTag("NotFound", (error) => enabled ? Effect.succeed("guest") : Effect.fail(error))
)

// Should NOT trigger: catchCause must re-fail with failCause.
export const wrongCauseRefail = causeProgram.pipe(
  Effect.catchCause((cause) => Cause.hasFails(cause) ? Effect.succeed("guest") : Effect.fail(cause))
)

// Should NOT trigger: assigning the parameter means the re-fail is not untouched.
export const reassigned = program.pipe(
  Effect.catch((error) => {
    error = otherError
    if (isNotFound(error)) return Effect.succeed("guest")
    return Effect.fail(error)
  })
)

// Should NOT trigger: a defaulted parameter is not the untouched catch input.
export const defaulted = program.pipe(
  Effect.catch((error = otherError) => isNotFound(error) ? Effect.succeed("guest") : Effect.fail(error))
)
