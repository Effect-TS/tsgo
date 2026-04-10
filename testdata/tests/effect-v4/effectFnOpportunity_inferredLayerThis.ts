// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "effectFn": ["span", "suggested-span", "inferred-span", "no-span", "untraced"]
      }
    ]
  }
}

// @filename: effectFnOpportunity_inferredLayerThis.ts
import { Effect, Layer, Context } from "effect"

export class MyService extends Context.Service<MyService, {
  log: (_what: string) => Effect.Effect<void>
}>()("MyService") {
    static layer = Layer.effect(this, Effect.gen(function*() {
        yield* Effect.log("log")
        return { log: (what: string) => Effect.log(what) }
    }))

    static layerSucceed = Layer.succeed(this)({
        log: (what: string) => Effect.log(what)
    })

    static layerSync = Layer.sync(this)(() => {
        return { log: (what: string) => Effect.log(what) }
    })
}
