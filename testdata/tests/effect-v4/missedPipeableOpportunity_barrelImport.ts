// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service"
      }
    ]
  }
}

// @filename: missedPipeableOpportunity_barrelImport.ts
// @effect-diagnostics missedPipeableOpportunity:warning
import * as EffectNs from "effect/Effect"
import * as OptionNs from "effect/Option"
import { Effect, Option } from "effect"

declare const e: Effect.Effect<Effect.Effect<number>>
declare const oo: Option.Option<Option.Option<number>>

// Should trigger: modules imported as namespaces
export const a1 = EffectNs.asVoid(EffectNs.flatten(e))
export const a2 = OptionNs.getOrUndefined(OptionNs.flatten(oo))

// Should trigger: modules imported by name from the effect barrel
export const b1 = Effect.asVoid(Effect.flatten(e))
export const b2 = Option.getOrUndefined(Option.flatten(oo))
