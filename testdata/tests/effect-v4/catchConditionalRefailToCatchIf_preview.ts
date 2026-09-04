// @effect-diagnostics *:off
// @effect-diagnostics catchConditionalRefailToCatchIf:warning
import { Data, Effect } from "effect"

class NotFound extends Data.TaggedError("NotFound")<{}> {}
class DatabaseError extends Data.TaggedError("DatabaseError")<{}> {}

declare const program: Effect.Effect<string, NotFound | DatabaseError>

export const recovered = program.pipe(
  Effect.catch((error) =>
    error instanceof NotFound ? Effect.succeed("guest") : Effect.fail(error)
  )
)
