// @effect-diagnostics *:off
// @effect-diagnostics preferSucceedSomeOrNone:warning
import { Effect, Option } from "effect"

export const none = Effect.succeed(Option.none())
export const some = Effect.succeed(Option.some(1))
