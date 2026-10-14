// @filename: package.json
{ "name": "@effect/test-app", "version": "1.0.0" }

// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "keyPatterns": [
          { "target": "error", "pattern": "identifier" }
        ]
      }
    ]
  }
}

// @filename: test.ts
// @effect-v3
// @effect-diagnostics deterministicKeys:error
import * as Data from "effect/Data"

export class ErrorA extends Data.TaggedError("ErrorA")<{}> {}

export class KeybindingsConfigError extends Data.TaggedError("KeybindingsConfigParseError")<{}> {}
