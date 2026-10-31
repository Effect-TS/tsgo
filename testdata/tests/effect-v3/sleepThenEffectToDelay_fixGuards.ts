// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics sleepThenEffectToDelay:warning
import { Duration, Effect, pipe } from "effect"

declare const sendRequest: Effect.Effect<void>

// Evaluation order: every form but the curried one evaluates the duration before the
// continuation, while Effect.delay(continuation, duration) evaluates the continuation first.
let next: Effect.Effect<string> = Effect.succeed("old")
const duration = (): "1 second" => {
  next = Effect.succeed("new")
  return "1 second"
}
let makeNext = (): Effect.Effect<string> => next

// MUST TRIGGER, no fix: the rewrite would read `next` before `duration()` reassigns it
export const reorderedDataFirst = Effect.zipRight(Effect.sleep(duration()), next)
export const reorderedAndThen = Effect.andThen(Effect.sleep(duration()), next)
export const reorderedPipeable = Effect.sleep(duration()).pipe(Effect.zipRight(next))
export const reorderedFreePipe = pipe(Effect.sleep(duration()), Effect.andThen(next))
export const reorderedReference = Effect.flatMap(Effect.sleep(duration()), makeNext)

// MUST TRIGGER, no fix (conservative): both the duration and the continuation are calls
export const reorderedCalls = Effect.zipRight(Effect.sleep(Duration.seconds(1)), Effect.succeed("x"))

// MUST TRIGGER: the curried form already evaluates the continuation first
// -> Effect.delay(next, duration())
export const curriedKeepsOrder = Effect.zipRight(next)(Effect.sleep(duration()))

// MUST TRIGGER: a stable duration cannot observe or change the continuation
// -> Effect.delay(next, "1 second")
export const stableDuration = Effect.zipRight(Effect.sleep("1 second"), next)

// MUST TRIGGER: a thunk reads `next` after the sleep in both forms
// -> Effect.delay(Effect.suspend(() => next), duration())
export const thunkReadsLater = Effect.flatMap(Effect.sleep(duration()), () => next)

// MUST TRIGGER: a parenthesized curried factory is replaced together with its application
// -> Effect.delay(sendRequest, "1 second")
export const parenthesizedCurried = (Effect.zipRight(sendRequest))(Effect.sleep("1 second"))
export const doublyParenthesizedCurried = ((Effect.andThen(sendRequest)))(Effect.sleep("1 second"))

// Mutable durations: sleep reads the duration when it is constructed, before the continuation
// is evaluated, while Effect.delay(continuation, duration) reads it afterwards.
const tupleDuration: [number, number] = [0, 0]
const objectDuration = Duration.seconds(0)
const primitiveDuration = "1 second"
const mutateDuration = () => {
  tupleDuration[0] = 1
  return Effect.void
}

// MUST TRIGGER, no fix: a const binding of a tuple can still be mutated by the continuation
export const mutableTupleDuration = Effect.zipRight(Effect.sleep(tupleDuration), mutateDuration())
export const mutableTuplePipe = Effect.sleep(tupleDuration).pipe(Effect.zipRight(mutateDuration()))

// MUST TRIGGER, no fix (conservative): a Duration value is an object, so it is treated the same way
export const objectValueDuration = Effect.zipRight(Effect.sleep(objectDuration), mutateDuration())

// MUST TRIGGER: a const binding of a primitive cannot change
// -> Effect.delay(mutateDuration(), primitiveDuration)
export const primitiveConstDuration = Effect.zipRight(Effect.sleep(primitiveDuration), mutateDuration())

// Invocation: the sequencer calls the continuation with the sleep result, Effect.suspend calls it
// with no arguments, which a non-arrow function can observe through `arguments` or `this`.
const countArguments = function() {
  return Effect.succeed(arguments.length)
}
const arrowThunk = () => Effect.succeed(1)
declare const declaredThunk: () => Effect.Effect<number>

// MUST TRIGGER, no fix: a reference to a function expression could read `arguments`
export const functionReferenceArguments = Effect.flatMap(Effect.sleep(0), countArguments)
export const functionReferenceArgumentsPipe = Effect.sleep(0).pipe(Effect.andThen(countArguments))

// MUST TRIGGER, no fix: a declared function may be bound to any function
export const declaredReference = Effect.flatMap(Effect.sleep(0), declaredThunk)

// MUST TRIGGER, no fix: an inline function expression that builds its effect could read `arguments`
export const functionExpressionArguments = Effect.flatMap(Effect.sleep(0), function() {
  return Effect.succeed(arguments.length)
})

// MUST TRIGGER: an arrow cannot observe how it is called
// -> Effect.delay(Effect.suspend(arrowThunk), 0)
export const arrowReference = Effect.flatMap(Effect.sleep(0), arrowThunk)
// -> Effect.delay(Effect.suspend(() => Effect.succeed(1)), 0)
export const inlineArrow = Effect.flatMap(Effect.sleep(0), () => Effect.succeed(1))

// MUST TRIGGER: a function expression that only returns a stable value is inlined
// -> Effect.delay(sendRequest, 0)
export const functionExpressionStable = Effect.flatMap(Effect.sleep(0), function() {
  return sendRequest
})

// MUST TRIGGER, no fix: a property access may be bound to any function
const holder = { thunk: () => Effect.succeed(1) }
export const propertyReference = Effect.flatMap(Effect.sleep(0), holder.thunk)

// MUST TRIGGER, no fix: a `let` binding may be reassigned to a non-arrow function
let letThunk = () => Effect.succeed(1)
export const letReference = Effect.flatMap(Effect.sleep(0), letThunk)
