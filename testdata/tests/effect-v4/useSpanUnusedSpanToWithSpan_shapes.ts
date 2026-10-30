// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics useSpanUnusedSpanToWithSpan:warning
import { Effect } from "effect"
import { useSpan } from "effect/Effect"
import type { Span } from "effect/Tracer"

declare const work: Effect.Effect<number>
declare const fetchUser: (id: string) => Effect.Effect<number>
declare const args: readonly [string, (span: Span) => Effect.Effect<number>]
let mutable = 0

// MUST TRIGGER, no fix: named (not renamed) import of useSpan; there is no namespace receiver
// to reuse for withSpan. The diagnostic covers the bare `useSpan` callee.
export const namedImport = useSpan("a", () => work)

// MUST TRIGGER: parenthesized callee; the diagnostic covers the callee inside the parentheses.
// fix: work.pipe(Effect.withSpan("a"))
export const parenthesizedCallee = (Effect.useSpan)("a", () => work)

// MUST TRIGGER: parenthesized body; the parentheses go with the body.
// fix: (work).pipe(Effect.withSpan("a"))
export const parenthesizedBody = Effect.useSpan("a", () => (work))

// MUST TRIGGER, no fix: explicit type arguments on useSpan have no withSpan counterpart.
export const typeArguments = Effect.useSpan<number, never, never>("a", () => work)

// MUST TRIGGER, no fix: the function expression reads its own name.
export const ownName = Effect.useSpan("a", function self(): Effect.Effect<number> {
  return self.length > 0 ? work : work
})

// MUST TRIGGER, no fix: the function expression reads its own `this`.
export const usesThis = Effect.useSpan("a", function(): Effect.Effect<number> {
  // @ts-expect-error this is implicitly any
  return Effect.succeed(String(this).length)
})

// MUST TRIGGER, no fix: the body reads a parameter that is reassigned, so evaluating it eagerly
// could observe another value.
export const reassignedParameter = (id: string) => {
  id = id.trim()
  return Effect.useSpan("a", () => fetchUser(id))
}

// MUST TRIGGER: an expression statement whose body starts with an identifier.
// fix: work.pipe(Effect.withSpan("a"))
export const identifierStatement = () => {
  const before = 1
  Effect.useSpan("a", () => work)
  return before
}

// MUST TRIGGER, no fix: the rewritten statement would start with `[`, continuing
// `const before = 1` on the previous line.
export const bracketStatement = () => {
  const before = 1
  Effect.useSpan("a", () => [work][before - 1])
  return before
}

// MUST TRIGGER, no fix: the rewritten statement would start with `(`, continuing
// `const before = 1` on the previous line.
export const conditionalStatement = () => {
  const before = 1
  Effect.useSpan("a", () => before > 0 ? work : work)
  return before
}

// MUST TRIGGER, no fix: the body eagerly reads a class declared after the call.
export const readsLaterClass = Effect.useSpan("a", () => Effect.succeed(LaterClass))

// MUST TRIGGER, no fix: the body eagerly reads an enum declared after the call.
export const readsLaterEnum = Effect.useSpan("a", () => Effect.succeed(LaterEnum.A))

// MUST TRIGGER: function declarations are hoisted with their value.
// fix: Effect.sync(laterFunction).pipe(Effect.withSpan("a"))
export const readsLaterFunction = Effect.useSpan("a", () => Effect.sync(laterFunction))

// MUST TRIGGER: globals are initialized before the module runs.
// fix: Effect.succeed(Math.PI).pipe(Effect.withSpan("a"))
export const readsGlobal = Effect.useSpan("a", () => Effect.succeed(Math.PI))

// MUST TRIGGER: bindings declared inside the body travel with it.
// fix: Effect.succeed(((value: number) => value)(1)).pipe(Effect.withSpan("a"))
export const readsLocalDeclaration = Effect.useSpan("a", () => Effect.succeed(((value: number) => value)(1)))

// MUST TRIGGER: a later binding read only in a type position is not evaluated.
// fix: Effect.succeed<typeof laterValue | null>(null).pipe(Effect.withSpan("a"))
export const readsLaterInType = Effect.useSpan("a", () => Effect.succeed<typeof laterValue | null>(null))

// MUST TRIGGER, no fix: a shorthand property eagerly reads a `let` binding.
export const readsLetShorthand = Effect.useSpan("a", () => Effect.succeed({ mutable }))

// MUST TRIGGER, no fix: a computed method name is evaluated eagerly and reads a later binding.
export const readsLaterInComputedName = Effect.useSpan("a", () => Effect.succeed({ [laterValue]() { return 1 } }))

// MUST TRIGGER (near-miss for the `arguments` exclusion): `arguments` of a nested non-arrow
// function inside a function expression callback.
// fix: Effect.sync(function() { return arguments.length }).pipe(Effect.withSpan("a"))
export const argumentsOfNestedFunction = Effect.useSpan("a", function() {
  return Effect.sync(function() {
    return arguments.length
  })
})

// MUST NOT TRIGGER: the next-line directive turns the rule off for this line only.
// @effect-diagnostics-next-line useSpanUnusedSpanToWithSpan:off
export const directiveOff = Effect.useSpan("a", () => work)

// MUST NOT TRIGGER: `typeof span` is a reference to the span.
export const typeofSpan = Effect.useSpan("a", (span) => Effect.succeed<typeof span | null>(null))

// MUST NOT TRIGGER: a shorthand property reads the span.
export const shorthandSpan = Effect.useSpan("a", (span) => Effect.succeed({ span }))

// MUST NOT TRIGGER: a parameter with a default value is outside the shape the rule reads.
export const defaultValue = Effect.useSpan("a", (span: Span | undefined = undefined) => work)

// MUST NOT TRIGGER: a rest parameter is outside the shape the rule reads.
export const restParameter = Effect.useSpan("a", (...spans: Array<Span>) => work)

// MUST NOT TRIGGER: spread arguments.
export const spreadArguments = Effect.useSpan(...args)

// MUST NOT TRIGGER: a function expression callback reads its own `arguments`.
export const argumentsOfCallback = Effect.useSpan("a", function() {
  return Effect.succeed(arguments.length)
})

// MUST NOT TRIGGER: a parenthesized arrow callback reads the enclosing function's `arguments`.
export const argumentsInParenthesizedArrow = function() {
  return Effect.useSpan("a", (() => Effect.succeed(arguments.length)))
}

const laterValue = "later"
class LaterClass {}
enum LaterEnum {
  A
}
function laterFunction() {
  return 1
}
