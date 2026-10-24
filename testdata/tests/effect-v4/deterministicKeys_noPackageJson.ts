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
// @effect-diagnostics deterministicKeys:error
import { Context, Data } from "effect"

export class MyService extends Context.Service<MyService, {}>()("WrongServiceKey") {}

export class MyError extends Data.TaggedError("WrongErrorKey")<{}> {}
