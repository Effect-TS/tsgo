// @filename: tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "plugins": [
      {
        "name": "@effect/language-service"
      }
    ]
  }
}

// @filename: issue179_nestedEffectGenThis.ts
import { Effect } from "effect"

function combine<T>(a: T, b: T): T { return a }

export class Repro {
    run(): Effect.Effect<number> {
        return Effect.gen({ self: this }, function* () {
            return yield* Effect.gen({ self: this }, function* () {
                return combine(1, 2)
            })
        })
    }
}
