// @filename: tsconfig.json
{
  "compilerOptions": {
    "types": ["node"],
    "plugins": [
      {
        "name": "@effect/language-service",
        "ignoreEffectErrorsInTscExitCode": true,
        "skipDisabledOptimization": true
      }
    ]
  }
}

// @filename: obsoleteMatchImport.ts
// @effect-v3
import { Effect } from "effect"

// Not flagged in v3: @effect/match is valid in Effect v3
// @ts-expect-error - @effect/match not installed in v3 fixture
import * as Match from "@effect/match"

export const program = Effect.sync(() => Match)
