import { Effect, Data } from "effect"

class ErrorA extends Data.TaggedError("ErrorA")<{ message: string }> {}
class ErrorB extends Data.TaggedError("ErrorB")<{ code: number }> {}

declare const effectWithErrors: Effect.Effect<number, ErrorA | ErrorB>

// Should trigger missingEffectError - assigning Effect with errors to Effect<number> (no errors)
export const missingAllErrors: Effect.Effect<number> = effectWithErrors

// Should trigger missingEffectError - missing ErrorB
export const missingErrorB: Effect.Effect<number, ErrorA> = effectWithErrors

// Should NOT trigger - errors match
export const errorsMatch: Effect.Effect<number, ErrorA | ErrorB> = effectWithErrors

// Should NOT trigger - no errors in source
declare const effectNoErrors: Effect.Effect<number>
export const noSourceErrors: Effect.Effect<number> = effectNoErrors
