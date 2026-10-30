// @effect-diagnostics *:off
// @effect-diagnostics zipViaNestedFlatMapToAll:warning
import { Effect, pipe } from "effect"
import * as Fx from "effect/Effect"

interface User {
  readonly id: string
}

declare const getUser: Effect.Effect<User>
declare const getOrders: Effect.Effect<ReadonlyArray<string>>
declare const audit: (value: unknown) => Effect.Effect<void>

// Flow shapes: the flatMap is found as a step of any piping flow, and the fix
// replaces the flow up to that step, keeping the steps around it.

// MUST TRIGGER: pipe(...) with steps before and after the flatMap
// fix: pipe(Effect.all([pipe(getUser, Effect.tap(audit)), getOrders]), Effect.asVoid)
export const functionPipeMiddle = pipe(
  getUser,
  Effect.tap(audit),
  Effect.flatMap((user) => Effect.map(getOrders, (orders) => [user, orders] as const)),
  Effect.asVoid
)

// MUST TRIGGER: pipeable with steps before and after the flatMap
// fix: Effect.all([getUser.pipe(Effect.tap(audit)), getOrders]).pipe(Effect.asVoid)
export const pipeableMiddle = getUser.pipe(
  Effect.tap(audit),
  Effect.flatMap((user) => Effect.map(getOrders, (orders) => [user, orders] as const)),
  Effect.asVoid
)

// MUST TRIGGER: curried flatMap applied to an earlier step
// fix: Effect.all([Effect.tap(getUser, audit), getOrders])
export const curriedAfterStep = Effect.flatMap((user: User) =>
  Effect.map(getOrders, (orders) => [user, orders] as const)
)(Effect.tap(getUser, audit))

// MUST TRIGGER: data-first flatMap passed as a later data-first argument
// fix: Effect.andThen(getUser, Effect.all([getUser, getOrders]))
export const laterDataFirstArgument = Effect.andThen(
  getUser,
  Effect.flatMap(getUser, (user) => Effect.map(getOrders, (orders) => [user, orders] as const))
)

// MUST TRIGGER: inner data-last map over a flow with an earlier step
// fix: Effect.all([getUser, Effect.tap(getOrders, audit)])
export const innerDataLastAfterStep = Effect.flatMap(getUser, (user) =>
  Effect.map((orders: ReadonlyArray<string>) => [user, orders] as const)(Effect.tap(getOrders, audit)))

// MUST TRIGGER, no quickfix: an Effect.fn pipeline step followed by another step
export const effectFnPipelineMiddle = Effect.fn("effectFnPipelineMiddle")(
  function*() {
    return yield* getUser
  },
  Effect.flatMap((user) => Effect.map(getOrders, (orders) => [user, orders] as const)),
  Effect.asVoid
)

// MUST TRIGGER: aliased module with a record combiner
// fix: Effect.all({ first: getUser, second: getOrders }) (through the file's Effect import)
export const aliasedRecord = Fx.flatMap(getUser, (user) => Fx.map(getOrders, (orders) => ({ first: user, second: orders })))

// MUST NOT TRIGGER: the same key twice, written once as a string literal (also TS1117)
export const duplicateKeys = Effect.flatMap(getUser, (user) =>
  Effect.map(getOrders, (orders) => ({ k: user, "k": orders })))

// Directives in the file.

// MUST NOT TRIGGER: disabled on the next line
// @effect-diagnostics-next-line zipViaNestedFlatMapToAll:off
export const disabledByDirective = Effect.flatMap(getUser, (user) =>
  Effect.map(getOrders, (orders) => [user, orders] as const))

// MUST TRIGGER (as an error): severity raised on the next line
// @effect-diagnostics-next-line zipViaNestedFlatMapToAll:error
export const raisedByDirective = Effect.flatMap(getUser, (user) =>
  Effect.map(getOrders, (orders) => [user, orders] as const))

// Hoisting the inner effect to the outer flow evaluates its eager part once,
// earlier: every binding it reads must hold the same value there.
declare const make: (...args: ReadonlyArray<unknown>) => number
let mutable = 1
var legacy = 1

// MUST NOT TRIGGER: a `var` binding is read eagerly
export const varEager = Effect.flatMap(getUser, (user) =>
  Effect.map(Effect.succeed(make(legacy)), (n) => [user, n] as const))

// MUST NOT TRIGGER: a `let` binding is read eagerly through a shorthand property
export const shorthandLet = Effect.flatMap(getUser, (user) =>
  Effect.map(Effect.succeed({ mutable }), (n) => [user, n] as const))

// MUST NOT TRIGGER: a class declared later is read eagerly
export const classAfter = Effect.flatMap(getUser, (user) =>
  Effect.map(Effect.succeed(make(LaterClass)), (n) => [user, n] as const))

// MUST NOT TRIGGER: an enum declared later is read eagerly
export const enumAfter = Effect.flatMap(getUser, (user) =>
  Effect.map(Effect.succeed(make(LaterEnum.A)), (n) => [user, n] as const))

// MUST NOT TRIGGER: a computed method name is evaluated eagerly
export const computedMethodName = Effect.flatMap(getUser, (user) =>
  Effect.map(Effect.succeed({ [laterKey]() { return 1 } }), (n) => [user, n] as const))

// MUST NOT TRIGGER: an unresolved name is never treated as stable
// @ts-expect-error missing is not declared
export const unresolved = Effect.flatMap(getUser, (user) => Effect.map(Effect.succeed(make(missing)), (n) => [user, n] as const))

// MUST NOT TRIGGER: a reassigned catch clause binding is read eagerly
export function reassignedCatch() {
  try {
    return getUser
  } catch (error) {
    error = undefined
    return Effect.flatMap(getUser, (user) => Effect.map(Effect.succeed(error), (e) => [user, e] as const))
  }
}

// MUST TRIGGER: a function declared later is hoisted with its value
// fix: Effect.all([getUser, Effect.succeed(make(laterFunction))])
export const functionAfter = Effect.flatMap(getUser, (user) =>
  Effect.map(Effect.succeed(make(laterFunction)), (n) => [user, n] as const))

// MUST TRIGGER: a binding declared later is only read inside a thunk
// fix: Effect.all([getUser, Effect.sync(() => laterKey)])
export const thunkReadingLater = Effect.flatMap(getUser, (user) =>
  Effect.map(Effect.sync(() => laterKey), (n) => [user, n] as const))

// MUST TRIGGER: a binding declared later is only read in a type position
// fix: Effect.all([getUser, Effect.succeed<typeof laterKey | undefined>(undefined)])
export const laterTypeArgument = Effect.flatMap(getUser, (user) =>
  Effect.map(Effect.succeed<typeof laterKey | undefined>(undefined), (n) => [user, n] as const))

// MUST TRIGGER: bindings declared inside the inner effect are local to it
// fix: Effect.all([getUser, Effect.succeed(((value: number) => value)(1))])
export const localDeclaration = Effect.flatMap(getUser, (user) =>
  Effect.map(Effect.succeed(((value: number) => value)(1)), (n) => [user, n] as const))

// MUST TRIGGER: a catch clause binding that is never reassigned is stable
// fix: Effect.all([getUser, Effect.succeed(error)])
export function stableCatch() {
  try {
    return getUser
  } catch (error) {
    return Effect.flatMap(getUser, (user) => Effect.map(Effect.succeed(error), (e) => [user, e] as const))
  }
}

const laterKey = "later"
class LaterClass {}
enum LaterEnum {
  A
}
function laterFunction() {}
mutable = legacy
