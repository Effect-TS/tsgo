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
// @effect-diagnostics deterministicKeys:error
import { Data } from "effect"

export class ErrorA extends Data.TaggedError("ErrorA")<{}> {}

export class KeybindingsConfigError extends Data.TaggedError("KeybindingsConfigParseError")<{}> {}
