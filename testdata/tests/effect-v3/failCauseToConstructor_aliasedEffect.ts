// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics failCauseToConstructor:warning
import { fail as causeFail } from "effect/Cause"
import { failCause, failCauseSync } from "effect/Effect"
import * as Fx from "effect/Effect"

declare const error: Error

// MUST TRIGGER: the fallback fix uses the file's Effect namespace binding -> Fx.fail(error)
export const eager = failCause(causeFail(error))

// MUST TRIGGER: lazy form with the namespace binding -> Fx.failSync(() => error)
export const lazy = failCauseSync(() => causeFail(error))
