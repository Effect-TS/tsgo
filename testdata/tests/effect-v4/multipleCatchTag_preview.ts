// @effect-diagnostics *:off
// @effect-diagnostics multipleCatchTag:warning
import { Data, Effect, pipe } from "effect"

class NotFound extends Data.TaggedError("NotFound")<{}> {}
class Unauthorized extends Data.TaggedError("Unauthorized")<{}> {}

declare const program: Effect.Effect<string, NotFound | Unauthorized>

export const preview = pipe(
  program,
  Effect.catchTag("NotFound", () => Effect.succeed("missing")),
  Effect.catchTag("Unauthorized", () => Effect.succeed("denied"))
)
