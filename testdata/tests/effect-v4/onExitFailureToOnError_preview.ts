// @effect-diagnostics *:off
import { Effect, Exit } from "effect"

declare const program: Effect.Effect<number, string>

program.pipe(
  Effect.onExit((exit) =>
    Exit.isFailure(exit)
      ? Effect.logError("failed", { cause: exit.cause })
      : Effect.void
  )
)
