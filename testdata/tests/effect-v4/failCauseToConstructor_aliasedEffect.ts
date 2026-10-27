// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics failCauseToConstructor:warning
import { Effect as Fx } from "effect"
import { fail as causeFail } from "effect/Cause"
import { failCause, failCauseSync } from "effect/Effect"

declare const error: Error

// MUST TRIGGER: the fallback fix uses the file's aliased Effect binding -> Fx.fail(error)
export const eager = failCause(causeFail(error))

// MUST TRIGGER: lazy form with the aliased binding -> Fx.failSync(() => error)
export const lazy = failCauseSync(() => causeFail(error))

void Fx
