// @effect-diagnostics *:off
// @effect-diagnostics preferEffectString:warning
import * as Str from "effect/String"

declare global { interface String { trim(): string } }
declare const value: string
value.trim()
value["trim"]()
value.toUpperCase()
Str.trim(value)
