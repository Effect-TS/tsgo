// @effect-v3
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

// @filename: missedPipeableOpportunity_callWithOptionals.ts
// @effect-diagnostics missedPipeableOpportunity:warning
import { identity } from "effect"
import * as Effect from "effect/Effect"

declare const signal: AbortSignal
const program = Effect.succeed(42)

// should trigger, the runner keeps its options outside of the pipe
// Effect.runPromise(program.pipe(Effect.orDie, Effect.asVoid), { signal })
export const runPromiseWithOptions = Effect.runPromise(Effect.asVoid(Effect.orDie(program)), { signal })

// should trigger
// Effect.runPromise(program.pipe(identity, identity), { signal })
export const runPromiseIdentityWithOptions = Effect.runPromise(identity(identity(program)), { signal })

// should trigger on the inner calls, even if the runner returns a function
// Effect.runCallback(program.pipe(Effect.orDie, Effect.asVoid), { onExit: () => {} })
export const runCallbackWithOptions = Effect.runCallback(Effect.asVoid(Effect.orDie(program)), { onExit: () => {} })
