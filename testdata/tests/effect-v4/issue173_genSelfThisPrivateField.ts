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

// @filename: issue173_genSelfThisPrivateField.ts
import { Effect, Exit, Scope } from "effect"

export class DirectYieldedThis {
  readonly #scope: Scope.Closeable = Scope.makeUnsafe()

  run(): Effect.Effect<void> {
    return Effect.gen({ self: this }, function* () {
      yield* Scope.close(this.#scope, Exit.void)
    })
  }
}

export class HoistedThisScope {
  readonly #scope: Scope.Closeable = Scope.makeUnsafe()

  run(): Effect.Effect<void> {
    return Effect.gen({ self: this }, function* () {
      const scope = this.#scope
      yield* Scope.close(scope, Exit.void)
    })
  }
}
