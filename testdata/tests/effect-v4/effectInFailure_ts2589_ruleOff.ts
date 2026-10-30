// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "diagnosticSeverity": {
          "effectInFailure": "off"
        }
      }
    ]
  }
}

// @filename: effectInFailure_ts2589_ruleOff.ts
import { Data, Effect } from "effect"

type TE<T> = Data.TaggedEnum<{
  A: { a: T }
  B: { b?: T }
}>

interface TEDefinition extends Data.TaggedEnum.WithGenerics<1> {
  readonly taggedEnum: TE<this["A"]>
}

type Wrap<T> = Data.TaggedEnum.Kind<TEDefinition, T>

export const program = Effect.try({
  try: () => 1,
  catch: () => Effect.fail<Wrap<Wrap<Wrap<Wrap<Wrap<Wrap<Wrap<Wrap<Wrap<Wrap<Wrap<Wrap<Wrap<Wrap<Wrap<Wrap<Wrap<Wrap<Wrap<Wrap<Wrap<Wrap<Wrap<Wrap<Wrap<string>>>>>>>>>>>>>>>>>>>>>>>>>>(undefined as any)
})
