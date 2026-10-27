// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics failCauseToConstructor:warning
import { die as causeDie, fail as causeFail } from "effect/Cause"
import { failCause, failCauseSync } from "effect/Effect"
import { pipe } from "effect/Function"

declare const error: Error
declare const defect: unknown

// MUST TRIGGER (report only): no Effect module binding is in scope, so the fallback
// fix would write an unbound Effect.die / Effect.fail / Effect.failSync
export const eager = failCause(causeDie(defect))
export const piped = pipe(error, causeFail, failCause)
export const lazy = failCauseSync(() => causeFail(error))
