// @effect-diagnostics *:off
// @effect-diagnostics flatMapToTap:warning
import { Effect } from "effect"

declare const source: Effect.Effect<number>

// MUST TRIGGER: a trailing comment on the prior pipe step survives the removed Effect.as step
export const trailingStepComment = source.pipe(Effect.flatMap((n) => Effect.log(n).pipe(
  Effect.map(() => 1), // Keep this explanation of the prior step.
  Effect.as(n)
)))

// MUST TRIGGER: statement and argument comments in a block-bodied callback are kept
export const blockComments = source.pipe(Effect.andThen((n) => {
  // Keep the callback's logging.
  console.log(n)
  return Effect.log(/* keep this argument */ n).pipe(Effect.map(() => 1), Effect.as(n))
}))
