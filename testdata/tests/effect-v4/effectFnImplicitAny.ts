// @filename: tsconfig.json
{
  "compilerOptions": {
    "noImplicitAny": true,
    "plugins": [
      {
        "name": "@effect/language-service"
      }
    ]
  }
}

// @filename: effectFnImplicitAny.ts
import { Effect } from "effect"

// should trigger: standalone Effect.fn callback falls back to Effect.fn's Args extends Array<any>
export const createCallToken = Effect.fn("CallTokenService.createCallToken")(
  function* (input) {
    return input
  }
)

// TypeScript baseline: standalone function also triggers noImplicitAny
export const createCallTokenPlain = function(input) {
  return input
}

// should trigger: multiple positional params without annotations
export const withTwoParams = Effect.fn(function* (a, b) {
  return [a, b] as const
})

// TypeScript baseline: multiple unannotated params also trigger noImplicitAny
export const withTwoParamsPlain = function(a, b) {
  return [a, b] as const
}

// should not trigger: contextual any does not fire in standard TypeScript either
declare const acceptsAny: (f: (input: any) => Effect.Effect<any>) => void

acceptsAny(
  Effect.fn("acceptsAny")(function* (input) {
    return input
  })
)

// TypeScript baseline: contextual any does not trigger noImplicitAny
acceptsAny(function(input) {
  return Effect.succeed(input)
})

// should not trigger: concrete contextual type for the returned function drives the param type
declare const acceptsString: (f: (input: string) => Effect.Effect<number>) => void

acceptsString(
  Effect.fn("acceptsString")(function* (input) {
    return input.length
  })
)

// TypeScript baseline: concrete contextual typing suppresses noImplicitAny
acceptsString(function(input) {
  return Effect.succeed(input.length)
})

// should not trigger: object destructuring is concretely typed by the receiving callback type
declare const acceptsRequest: (f: (input: { readonly id: string }) => Effect.Effect<string>) => void

acceptsRequest(
  Effect.fn("acceptsRequest")(function* ({ id }) {
    return id
  })
)

// TypeScript baseline: destructured param is typed by the receiving callback type
acceptsRequest(function({ id }) {
  return Effect.succeed(id)
})

// should not trigger: higher-order make-style contextual typing should win over Effect.fn fallback
declare const make: (f: (input: string) => Effect.Effect<number>) => void

make(
  Effect.fn("make")(function* (input) {
    return input.length
  })
)

// TypeScript baseline: higher-order contextual typing should also suppress noImplicitAny
make(function(input) {
  return Effect.succeed(input.length)
})
