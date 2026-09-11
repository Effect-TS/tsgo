// @effect-diagnostics *:off
// @effect-diagnostics catchAllTagDispatchToCatchTag:suggestion
import { Data, Effect } from "effect"

class NotFound extends Data.TaggedError("NotFound")<{}> {}
class Timeout extends Data.TaggedError("Timeout")<{}> {}
class Unauthorized extends Data.TaggedError("Unauthorized")<{}> {}

declare const program: Effect.Effect<string, NotFound | Timeout | Unauthorized>

export const recovered = program.pipe(
  Effect.catch((error) => {
    switch (error._tag) {
      case "NotFound":
        return Effect.succeed("guest")
      case "Timeout":
        return Effect.succeed("retrying")
      default:
        return Effect.fail(error)
    }
  })
)
