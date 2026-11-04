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

// @filename: effectFnOpportunity_classExtendsHoisting.ts
import { Context, Effect } from "effect"

// A class extends expression runs when the class is defined, so it relies on
// the hoisting of this helper: no opportunity or conversion.
export class Early extends Context.Service<Early, { readonly value: number }>()("Early", {
  make: makeEarly(1)
}) {}

function makeEarly(n: number) {
  return Effect.gen(function* () {
    const value = yield* Effect.succeed(n)
    return { value: value + 1 }
  })
}

// The same holds for any call in a class extends expression.
const base = <T>(make: T) => class { static make = make }

export class EarlyBase extends base(viaBase(1)) {}

function viaBase(n: number) {
  return Effect.gen(function* () {
    const value = yield* Effect.succeed(n)
    return value + 1
  })
}

// Type arguments of the extends clause do not evaluate the function, so the
// opportunity for typeOnly is kept.
declare const Generic: new <T>() => { readonly marker?: T }

export class TypeOnly extends Generic<typeof typeOnly> {}

function typeOnly(n: number) {
  return Effect.gen(function* () {
    const value = yield* Effect.succeed(n)
    return value + 1
  })
}
