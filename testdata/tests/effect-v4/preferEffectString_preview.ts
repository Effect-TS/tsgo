// @effect-diagnostics *:off
// @effect-diagnostics preferEffectString:warning
import * as Str from "effect/String"

const email = "alice@example.com"
export const native = email.indexOf("@")
export const preferred = Str.indexOf("@")(email)
