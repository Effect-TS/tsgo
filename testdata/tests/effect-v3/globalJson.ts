// @effect-v3
// @effect-diagnostics globalJson:warning
import { Effect, Schema } from "effect"

const User = Schema.Struct({ name: Schema.String })

// Should trigger - JSON.parse feeding a Schema decoder outside Effect
export const parseUser = (text: string) => Schema.decodeUnknownSync(User)(JSON.parse(text))

// Should trigger - JSON.parse throws instead of returning a failed result
export const parseUserEither = (text: string) => Schema.decodeUnknownEither(User)(JSON.parse(text))

// Should trigger - JSON.stringify at module level
export const serialized = JSON.stringify({ name: "John" })

// Should trigger - aliased JSON
const MyJSON = JSON
export const aliasedJson = MyJSON.parse("{}")

// Should trigger - aliased JSON.parse method
export const aliasedParse = (text: string) => {
  const parse = JSON.parse
  return parse(text)
}

// Should NOT trigger - JSON.parse inside Effect.gen
export const parseInGen = Effect.gen(function*() {
  return JSON.parse("{}")
})

// Should trigger - JSON.parse inside nested arrow in generator
export const nestedArrowInGen = Effect.gen(function*() {
  const fn = (text: string) => JSON.parse(text)
  return fn
})

// Should NOT trigger - shadowed JSON
const _shadowedJson = (text: string) => {
  const JSON = { parse: (value: string) => value, stringify: (value: unknown) => String(value) }
  return JSON.stringify(JSON.parse(text))
}

// Should NOT trigger - other JSON members
export const toStringTag = JSON[Symbol.toStringTag]
