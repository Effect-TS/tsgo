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
// @effect-v3
// @effect-diagnostics deterministicKeys:error
import * as Context from "effect/Context"
import * as Data from "effect/Data"

export class ExpectedServiceIdentifier
  extends Context.Tag("ExpectedServiceIdentifier")<ExpectedServiceIdentifier, {}>()
{}

export class ErrorA extends Data.TaggedError("ErrorA")<{}> {}
