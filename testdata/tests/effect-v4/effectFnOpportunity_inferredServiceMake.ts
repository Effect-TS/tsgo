// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "effectFn": ["inferred-span"]
      }
    ]
  }
}

// @filename: effectFnOpportunity_inferredServiceMake.ts
import { Effect, Layer, Context } from "effect"

export class UserService extends Context.Service<UserService>()("UserService", {
    make: Effect.gen(function*() {
        return {
            getUser: (id: string) =>
                Effect.gen(function*() {
                    yield* Effect.log(`Looking up user ${id}`)
                })
        }
    })
}) {}
