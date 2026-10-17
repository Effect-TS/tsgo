// @filename: tsconfig.json
{"compilerOptions":{"plugins":[{"name":"@effect/language-service","effectFn":["span","untraced","no-span","inferred-span","suggested-span"]}]}}

// @filename: effectFnOpportunity_layerComments.ts
import { Context, Effect, Layer } from "effect"

class Store extends Context.Service<Store, {
  readonly size: number
  readonly get: (key: string) => Effect.Effect<string>
}>()("Store") {}

export const layer = Layer.effect(
  Store,
  Effect.gen(function* () {
    const prefix = yield* Effect.succeed("k:")
    return {
      // how many entries the store holds
      size: 1_000,
      get: (key: string) =>
        Effect.gen(function* () {
          // look the key up
          return prefix + key
        }),
    }
  }),
)
