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

// @filename: effectFnOpportunity_hoistedDeclaration.ts
import { Effect } from "effect"

// Issue #758, point 4: no opportunity or conversion for this hoisted helper.
export const program = helper(1)

function helper(n: number) {
  return Effect.gen(function* () {
    return yield* Effect.succeed(n)
  })
}

// Taking the function's value before its declaration also relies on hoisting.
export const earlyObject = { shorthand }

function shorthand(n: number) {
  return Effect.gen(function* () {
    const value = yield* Effect.succeed(n)
    return value + 1
  })
}

// The same restriction applies inside a function body.
export function enclosing(n: number) {
  const result = local(n)
  return result

  function local(value: number) {
    return Effect.gen(function* () {
      const result = yield* Effect.succeed(value)
      return result + 1
    })
  }
}

// Earlier closures can execute before the declaration is reached.
const invokeCaptured = () => captured(1)
export const earlyClosure = invokeCaptured()

function captured(n: number) {
  return Effect.gen(function* () {
    const value = yield* Effect.succeed(n)
    return value + 1
  })
}

// Non-generator conversions must also preserve hoisting.
export const earlyRegular = regular(1)

function regular(n: number) {
  const a = n + 1
  const b = a + 1
  const c = b + 1
  const d = c + 1
  const e = d + 1
  return Effect.succeed(e)
}

// These are different symbols and must not suppress the opportunity for safe.
export const shadowed = (safe: number) => safe
export const property = { safe: 1 }

export function safe(n: number) {
  return Effect.gen(function* () {
    const value = yield* Effect.succeed(n)
    return value + 1
  })
}

// Type-only references and re-exports do not evaluate the function's value.
export type LaterSignature = typeof later
export { later }

function later(n: number) {
  return Effect.gen(function* () {
    const value = yield* Effect.succeed(n)
    return value + 1
  })
}

// A reference after the declaration must still allow conversion.
export const laterProgram = later(1)
