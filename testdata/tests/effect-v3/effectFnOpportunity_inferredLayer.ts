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

// @filename: effectFnOpportunity_inferredLayer.ts
// @effect-v3
import * as Context from "effect/Context"
import * as Effect from "effect/Effect"
import * as Layer from "effect/Layer"

class MyService extends Context.Tag("MyService")<MyService, {
    log: (what: string) => Effect.Effect<void>
}
>(){}

const _shouldTrigger = Layer.effect(MyService)(Effect.gen(function*() {
    yield* Effect.log("log")
    // Log should be inferred with name MyService.log
    return { log: (what: string) => Effect.log(what) }
}))

const _shouldTriggerConstructor = Layer.effect(MyService, Effect.gen(function*() {
    yield* Effect.log("log")
    // Log should be inferred with name MyService.log
    return { log: (what: string) => Effect.log(what) }
}))

const _shouldTriggerSucceed = Layer.succeed(MyService)({
    // Log should be inferred with name MyService.log
    log: (what: string) => Effect.log(what)
})

const _shouldTriggerSync = Layer.sync(MyService)(() => {
    // Log should be inferred with name MyService.log
    return { log: (what: string) => Effect.log(what) }
})
