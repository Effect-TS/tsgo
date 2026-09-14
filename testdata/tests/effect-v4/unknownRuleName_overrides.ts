// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "overrides": [
          {
            "include": ["**/*.ts"],
            "options": {
              "diagnosticSeverity": {
                "floatingEfect": "error"
              }
            }
          }
        ]
      }
    ]
  }
}

// @filename: test.ts
import { Effect } from "effect"

// diagnosticSeverity inside an overrides entry is checked the same way, and a
// near miss carries the intended name.
export const program = Effect.succeed(1)
