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

// @filename: /node_modules/react/package.json
{ "name": "react", "version": "19.0.0", "types": "index.d.ts" }

// @filename: /node_modules/react/index.d.ts
export declare function useRef<T>(value: T): { current: T }

// @filename: missedPipeableOpportunity_hooks.ts
// @effect-diagnostics missedPipeableOpportunity:warning
import { Effect } from "effect"
import * as React from "react"

declare function useEffectValue<A>(effect: Effect.Effect<A>): A
declare function use2D<A>(effect: Effect.Effect<A>): A
declare function useful<A>(effect: Effect.Effect<A>): A

export function Component() {
  // should not trigger: a hook ends the pipeable run
  const a = useEffectValue(Effect.asVoid(Effect.succeed(1)))
  // should not trigger: hook names may continue with a digit
  const b = use2D(Effect.asVoid(Effect.succeed(1)))
  // should not trigger: hooks read from a namespace
  const c = React.useRef(Effect.asVoid(Effect.succeed(1)))
  // should trigger on the run before the hook
  const d = useEffectValue(Effect.asVoid(Effect.asVoid(Effect.succeed(1))))
  // should trigger: `useful` is not a hook name
  const e = useful(Effect.asVoid(Effect.succeed(1)))
  return [a, b, c, d, e]
}
