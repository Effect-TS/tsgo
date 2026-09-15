// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "diagnosticSeverity": {
          "floatingEffect": "error",
          "importFromBarrel": "error",
          "outdatedEffectCodegen": "error"
        }
      }
    ]
  }
}

// @filename: test.ts
import { Effect } from "effect"

// The rule names above that this build does not provide are reported on the
// tsconfig; floatingEffect is provided, so it is not.
export const program = Effect.succeed(1)
