// @filename: /app/tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "keyPatterns": [
          { "target": "service", "pattern": "default" },
          { "target": "error", "pattern": "identifier" }
        ]
      }
    ]
  }
}

// @filename: /app/src/services/test.ts
// @effect-v3
// @effect-diagnostics deterministicKeys:error
import * as Context from "effect/Context"
import * as Data from "effect/Data"

export class MyService extends Context.Tag("WrongServiceKey")<MyService, {}>() {}

export class MyError extends Data.TaggedError("WrongErrorKey")<{}> {}
