// @effect-diagnostics globalJsonInEffect:warning
import { Effect, Schema } from "effect"

const User = Schema.Struct({ name: Schema.String })

// Should trigger - JSON.parse inside Effect.gen
export const parseUserInGen = (text: string) =>
  Effect.gen(function*() {
    const json = JSON.parse(text)
    return yield* Schema.decodeUnknownEffect(User)(json)
  })

// Should trigger - JSON.stringify inside Effect.gen
export const stringifyInGen = Effect.gen(function*() {
  const data = yield* Effect.succeed({ name: "Test" })
  return JSON.stringify(data)
})

// Should trigger - JSON.parse inside Effect.fn
export const parseInFn = Effect.fn("parseInFn")(function*(text: string) {
  return JSON.parse(text)
})

// Should trigger - aliased JSON
export const aliasedJson = Effect.gen(function*() {
  const MyJSON = JSON
  return MyJSON.parse("{}")
})

// Should NOT trigger - JSON.parse at module level
const _moduleLevel = JSON.parse("{}")

// Should NOT trigger - JSON.parse in regular function
const _regularFn = (text: string) => JSON.parse(text)

// Should NOT trigger - JSON.parse inside nested arrow in generator
export const nestedArrowInGen = Effect.gen(function*() {
  const fn = (text: string) => JSON.parse(text)
  return fn
})

// Should NOT trigger - shadowed JSON
export const shadowedJson = Effect.gen(function*() {
  const JSON = { parse: (value: string) => value }
  return JSON.parse("{}")
})
