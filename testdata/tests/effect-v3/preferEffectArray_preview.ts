// @effect-diagnostics *:off
// @effect-diagnostics preferEffectArray:warning
import * as A from "effect/Array"

const values = [1, 2, 3]
export const native = values.find(n => n > 1)
export const preferred = A.findFirst(values, n => n > 1)
