// @effect-diagnostics *:off
// @effect-diagnostics isTaggedOnTaggedUnion:warning
import { Effect, Predicate } from "effect"

// Option is not imported, so the fix is Predicate.isTaggedStrict instead of Option.isSome.
const option = Effect.runSync(Effect.option(Effect.succeed(1)))

export const some = Predicate.isTagged(option, "Some")
