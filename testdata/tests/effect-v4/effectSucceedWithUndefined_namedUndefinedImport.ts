// @effect-diagnostics *:off
// @effect-diagnostics effectSucceedWithUndefined:suggestion
import { succeed, undefined as undefinedEffect } from "effect/Effect"

// MUST TRIGGER: no Effect module binding, but the replacement can still reach Effect.undefined
export const result = succeed(undefined)

export const used = undefinedEffect
