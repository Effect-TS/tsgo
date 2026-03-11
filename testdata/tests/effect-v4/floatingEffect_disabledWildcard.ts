import { Effect } from "effect"

Effect.succeed(1)

/** @effect-diagnostics *:off */

Effect.succeed(1)

/** @effect-diagnostics floatingEffect:error */
Effect.succeed(1)

/** @effect-diagnostics *:off */
Effect.succeed(1)
