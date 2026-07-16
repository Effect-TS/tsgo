import { Effect } from "effect"

const constant = "constant"
const objectConstant = { value: "constant" }
let mutable = "constant"

// Should trigger: primitive literals
export const stringLiteral = Effect.sync(() => "value")
export const numberLiteral = Effect.sync(() => 1)
export const booleanLiteral = Effect.sync(() => true)
export const nullLiteral = Effect.sync(() => null)
export const undefinedLiteral = Effect.sync(() => undefined)
export const templateLiteral = Effect.sync(() => `value`)

// Should trigger: an initialized const binding
export const constIdentifier = Effect.sync(() => constant)
export const blockBody = Effect.sync(function() {
  return constant
})

// Should NOT trigger: these expressions are evaluated or allocated per execution
export const callExpression = Effect.sync(() => Date.now())
export const objectLiteral = Effect.sync(() => ({ value: "constant" }))
export const arrayLiteral = Effect.sync(() => [constant])
export const propertyRead = Effect.sync(() => objectConstant.value)
// @effect-diagnostics-next-line lazyPromiseInEffectSync:off
export const asyncThunk = Effect.sync(async () => "value")
export const generatorThunk = Effect.sync(function*() {
  return "value"
})

// Should NOT trigger: mutable or not initialized before construction
export const mutableIdentifier = Effect.sync(() => mutable)
export const declaredLater = Effect.sync(() => later)
const later = "later"

export const switchCase = (value: number) => {
  switch (value) {
    case 0:
      const caseConstant = "value"
      return caseConstant
    default:
      return Effect.sync(() => caseConstant)
  }
}

// Should NOT trigger: unrelated sync API
const unrelated = { sync: <A>(thunk: () => A): A => thunk() }
export const unrelatedSync = unrelated.sync(() => "value")
