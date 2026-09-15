// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "diagnosticSeverity": {
          "floatingEffect": "error",
          "unusedDirective": "warning",
          "unknownRuleName": "warning"
        }
      }
    ]
  }
}

// @filename: test.ts
import { Effect } from "effect"

// Every configured name resolves, so the configuration is reported clean.
export const program = Effect.succeed(1)
