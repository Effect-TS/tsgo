// @effect-diagnostics *:off
// @effect-diagnostics isTaggedOnTaggedUnion:warning
import { Effect, Predicate } from "effect"

// Option is not imported, so the fix compares the tag instead of using Option.isSome.
const option = Effect.runSync(Effect.option(Effect.succeed(1)))

export const some = Predicate.isTagged(option, "Some")
