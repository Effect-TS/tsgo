// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics flatMapIdentityToFlatten:warning
import { Array, Effect, Fiber, Function, identity, Option, pipe, Stream } from "effect"
import * as Fx from "effect/Effect"
import { identity as id } from "effect/Function"

class LoadError {
  readonly _tag = "LoadError"
}
class RenderError {
  readonly _tag = "RenderError"
}
class Db {
  readonly _tag = "Db"
}

declare const loadTask: Effect.Effect<Effect.Effect<string>>
declare const failingTask: Effect.Effect<Effect.Effect<string, RenderError, Db>, LoadError>
declare const fiber: Fiber.Fiber<number, LoadError>
declare const fallback: Effect.Effect<string>

// MUST TRIGGER: pipeable style with an arrow identity callback
export const pipeableArrow = loadTask.pipe(Effect.flatMap((inner) => inner))

// MUST TRIGGER: pipeable style with the `identity` export
export const pipeableIdentity = loadTask.pipe(Effect.flatMap(identity))

// MUST TRIGGER: pipe function style
export const piped = pipe(loadTask, Effect.flatMap((inner) => inner))

// MUST TRIGGER: data-first style with the `identity` export
export const dataFirstIdentity = Effect.flatMap(loadTask, identity)

// MUST TRIGGER: data-first style with an arrow identity callback
export const dataFirstArrow = Effect.flatMap(loadTask, (inner) => inner)

// MUST TRIGGER: data-last style
export const dataLast = Effect.flatMap((inner: Effect.Effect<string>) => inner)(loadTask)

// MUST TRIGGER: annotated parameter is still an identity
export const annotated = loadTask.pipe(Effect.flatMap((inner: Effect.Effect<string>) => inner))

// MUST TRIGGER: parenthesized return of the parameter
export const parenthesized = loadTask.pipe(Effect.flatMap((inner) => (inner)))

// MUST TRIGGER: block body whose only statement returns the parameter
export const singleReturnBlock = loadTask.pipe(Effect.flatMap((inner) => {
  return inner
}))

// MUST TRIGGER: function expression returning its parameter
export const functionExpression = loadTask.pipe(Effect.flatMap(function(inner) {
  return inner
}))

// MUST TRIGGER: identity referenced through the Function module namespace
export const functionNamespace = loadTask.pipe(Effect.flatMap(Function.identity))

// MUST TRIGGER: identity imported from effect/Function under an alias
export const aliasedIdentity = loadTask.pipe(Effect.flatMap(id))

// MUST TRIGGER: Effect module imported under an alias
export const aliasedModule = loadTask.pipe(Fx.flatMap((inner) => inner))

// MUST TRIGGER: error and requirement channels on both levels are merged by flatten too
export const withChannels = failingTask.pipe(Effect.flatMap((inner) => inner))

// MUST TRIGGER: Exit is an Effect, so unwrapping Fiber.await is Effect.flatten
export const fiberExit = Fiber.await(fiber).pipe(Effect.flatMap((exit) => exit))

// MUST TRIGGER: in v3 Option is an Effect subtype, so this unwrap is Effect.flatten too
declare const optionTask: Effect.Effect<Option.Option<number>>
export const optionSubtype = optionTask.pipe(Effect.flatMap((option) => option))

// MUST TRIGGER: identity step in the middle of a longer flow
export const longerFlow = loadTask.pipe(
  Effect.tap(() => Effect.log("loaded")),
  Effect.flatMap((inner) => inner),
  Effect.map((report) => report.length)
)

// MUST TRIGGER: generic nested effect
export const generic = <A, E, R>(self: Effect.Effect<Effect.Effect<A, E, R>>) =>
  self.pipe(Effect.flatMap((inner) => inner))

// MUST NOT TRIGGER: already using Effect.flatten
export const alreadyFlatten = Effect.flatten(loadTask)
export const alreadyFlattenPipe = loadTask.pipe(Effect.flatten)

// MUST NOT TRIGGER: type assertion makes the callback more than an identity
export const typeAssertion = loadTask.pipe(
  Effect.flatMap((inner) => inner as Effect.Effect<string, unknown>)
)

// MUST NOT TRIGGER: block body with additional statements
export const extraStatements = loadTask.pipe(Effect.flatMap((inner) => {
  console.log("unwrapping")
  return inner
}))

// MUST NOT TRIGGER: callback returns a different binding
export const differentBinding = loadTask.pipe(Effect.flatMap((_inner) => fallback))

// MUST NOT TRIGGER: inner parameter shadows the outer one, the outer is never returned
export const shadowed = loadTask.pipe(Effect.flatMap((inner) => ((inner: Effect.Effect<string>) => inner)(fallback)))

// MUST NOT TRIGGER: callback transforms the inner effect
export const transformed = loadTask.pipe(
  Effect.flatMap((inner) => inner.pipe(Effect.map((report) => report.toUpperCase())))
)

// MUST NOT TRIGGER: property access on the parameter
declare const holderTask: Effect.Effect<{ readonly task: Effect.Effect<string> }>
export const propertyAccess = holderTask.pipe(Effect.flatMap((holder) => holder.task))

// MUST NOT TRIGGER: user-defined identity function is not effect's `identity` export
const localIdentity = <A>(a: A): A => a
export const userIdentity = loadTask.pipe(Effect.flatMap(localIdentity))

// MUST NOT TRIGGER: success channel is `any`, not an Effect
declare const anyTask: Effect.Effect<any>
export const anySuccess = anyTask.pipe(Effect.flatMap((value) => value))

// MUST NOT TRIGGER: success channel is `never`, not an Effect
export const neverSuccess = Effect.fail(new LoadError()).pipe(Effect.flatMap((value) => value))

// MUST NOT TRIGGER: identity map is a different (no-op) operation
export const identityMap = loadTask.pipe(Effect.map((inner) => inner))

// MUST NOT TRIGGER: andThen is outside this rule's scope
export const identityAndThen = loadTask.pipe(Effect.andThen((inner) => inner))

// MUST NOT TRIGGER: Option.flatMap with identity is Option.flatten, not Effect
declare const nestedOption: Option.Option<Option.Option<number>>
export const optionFlatMap = Option.flatMap(nestedOption, identity)
export const optionFlatMapPipe = nestedOption.pipe(Option.flatMap((inner) => inner))

// MUST NOT TRIGGER: Array.flatMap with identity is Array.flatten, not Effect
declare const nestedArray: ReadonlyArray<ReadonlyArray<number>>
export const arrayFlatMap = Array.flatMap(nestedArray, identity)

// MUST NOT TRIGGER: native Array.prototype.flatMap
export const nativeFlatMap = [[1], [2]].flatMap((inner) => inner)

// MUST NOT TRIGGER: Stream.flatMap with identity is Stream.flatten, not Effect
declare const nestedStream: Stream.Stream<Stream.Stream<number>>
export const streamFlatMap = nestedStream.pipe(Stream.flatMap((inner) => inner))

// MUST NOT TRIGGER: unrelated flatMap API
const unrelated = { flatMap: <A, B>(value: A, f: (value: A) => B): B => f(value) }
export const unrelatedFlatMap = unrelated.flatMap(loadTask, (inner) => inner)
