// @effect-diagnostics *:off
// @effect-diagnostics catchIfTagToCatchTag:suggestion
import { Data, Effect } from "effect"

class NotFound extends Data.TaggedError("NotFound")<{}> {}
class Timeout extends Data.TaggedError("Timeout")<{}> {}

declare const program: Effect.Effect<string, NotFound | Timeout>

export const recovered = program.pipe(
  Effect.catchIf(
    (error) => error._tag === "NotFound",
    () => Effect.succeed("guest")
  )
)
