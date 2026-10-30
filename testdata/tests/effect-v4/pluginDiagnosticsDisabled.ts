// @filename: tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "plugins": [
      {
        "name": "@effect/language-service",
        "diagnostics": false,
        "diagnosticSeverity": {
          "floatingEffect": "error"
        }
      }
    ]
  }
}

// @filename: pluginDiagnosticsDisabled.ts
import { Effect } from "effect"

Effect.succeed(1)
