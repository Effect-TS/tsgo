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
import { Effect, Layer, Context } from "effect"

class MyService extends Context.Service<MyService, {
  log: (_what: string) => Effect.Effect<void>
}>()("MyService") {}

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
