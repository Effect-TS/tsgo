// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "debug": true,
        "ignoreEffectErrorsInTscExitCode": true,
        "skipDisabledOptimization": true
      }
    ]
  }
}

// @Filename: /test.ts
import { Effect } from "effect"

declare const start: Effect.Effect<number>

export const flow = start.pipe(
  Effect.map((n) => n + 1),
  Effect.flatMap((n) => Effect.succeed(String(n)))
)
