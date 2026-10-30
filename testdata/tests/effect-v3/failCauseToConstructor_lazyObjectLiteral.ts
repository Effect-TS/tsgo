// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics failCauseToConstructor:warning
import { Cause, Effect } from "effect"

// MUST TRIGGER: a bare object literal as a concise arrow body would parse as a block,
// so the fix keeps it parenthesized -> Effect.failSync(() => ({ _tag: "E" }))
export const lazyObjectLiteral = Effect.failCauseSync(() => Cause.fail({ _tag: "E" }))
