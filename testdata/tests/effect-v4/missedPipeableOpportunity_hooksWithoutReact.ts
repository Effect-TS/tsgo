// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "pipeableMinArgCount": 2
      }
    ]
  }
}

// @filename: missedPipeableOpportunity_hooksWithoutReact.ts
// @effect-diagnostics missedPipeableOpportunity:warning
import { Effect } from "effect"

declare function useEffectValue<A>(effect: Effect.Effect<A>): A

// should trigger: the program does not reference React, so `use*` names are not hooks
export const a = useEffectValue(Effect.asVoid(Effect.succeed(1)))
