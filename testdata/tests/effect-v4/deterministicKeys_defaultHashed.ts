// @filename: package.json
{ "name": "@effect/test-app", "version": "1.0.0" }

// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "keyPatterns": [
          { "target": "service", "pattern": "default-hashed" },
          { "target": "error", "pattern": "default-hashed" }
        ]
      }
    ]
  }
}

// @filename: test.ts
// @effect-diagnostics deterministicKeys:error
import { Context, Data } from "effect"

export class ExpectedServiceIdentifier
  extends Context.Service<ExpectedServiceIdentifier, {}>()("ExpectedServiceIdentifier")
{}

export class ErrorA extends Data.TaggedError("ErrorA")<{}> {}
