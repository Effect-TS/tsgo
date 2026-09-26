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

// @filename: effectFnOpportunity_nestedInitializer.ts
import { Effect } from "effect"

declare const define: <A>(options: {
  readonly name: string
  readonly decode: (value: string) => Effect.Effect<A>
}) => A

export const configuration = define({
  name: "workspace",
  decode: (value) =>
    Effect.gen(function*() {
      const text = yield* Effect.succeed(value)
      return text.length
    }),
})

interface Definition<A> {
  readonly read: (path: string) => Effect.Effect<A>
}

export const make = <A>(value: A): Definition<A> => ({
  read: (path) =>
    Effect.gen(function*() {
      yield* Effect.log(path)
      return value
    }),
})
