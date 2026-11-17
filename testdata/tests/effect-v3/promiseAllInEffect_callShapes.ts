// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics promiseAllInEffect:warning
import { Effect } from "effect"

interface User {
  readonly name: string
}

declare const ids: ReadonlyArray<string>
declare const pairIds: readonly [string, string]
declare const entries: ReadonlyArray<{ readonly id: string }>
declare const fetchUser: (id: string) => Promise<User>
declare const fetchUserWithSignal: (id: string, signal: AbortSignal) => Promise<User>
declare const idsFor: (signal: AbortSignal) => ReadonlyArray<string>
declare const cachedUser: User
declare const maybeIds: ReadonlyArray<string | undefined>

// ---------------------------------------------------------------------------
// MUST TRIGGER, quick fix: call shapes around the thunk-safe form.
// ---------------------------------------------------------------------------

// MUST TRIGGER: the steps after Effect.promise(...) are kept.
// Quick fix: Effect.forEach(ids, (id) => Effect.promise(() => fetchUser(id)), { concurrency: "unbounded" }).pipe(Effect.asVoid)
export const trailingPipe = Effect.promise(() => Promise.all(ids.map((id) => fetchUser(id)))).pipe(Effect.asVoid)

// MUST TRIGGER: an annotated thunk parameter is redeclared as written.
// Quick fix: Effect.forEach(ids, (id) => Effect.promise((signal: AbortSignal) => fetchUserWithSignal(id, signal)), { concurrency: "unbounded" })
export const annotatedSignal = Effect.promise((signal: AbortSignal) =>
  Promise.all(ids.map((id) => fetchUserWithSignal(id, signal)))
)

// MUST TRIGGER: the callback's return type annotation moves to the per-item thunk.
// Quick fix: Effect.forEach(ids, (id) => Effect.promise((): Promise<User> => fetchUser(id)), { concurrency: "unbounded" })
export const annotatedReturn = Effect.promise(() => Promise.all(ids.map((id): Promise<User> => fetchUser(id))))

// MUST TRIGGER: a destructured callback parameter, with no thunk parameter to collide with.
// Quick fix: Effect.forEach(entries, ({ id }) => Effect.promise(() => fetchUser(id)), { concurrency: "unbounded" })
export const destructuredParameter = Effect.promise(() => Promise.all(entries.map(({ id }) => fetchUser(id))))

// MUST TRIGGER: async function expression callback keeps `async` on the per-item thunk.
// Quick fix: Effect.forEach(ids, (id) => Effect.tryPromise(async () => { const user = await fetchUser(id); return user.name }), { concurrency: "unbounded" })
export const asyncFunctionExpression = Effect.tryPromise(() =>
  Promise.all(ids.map(async function(id) {
    const user = await fetchUser(id)
    return user.name
  }))
)

// ---------------------------------------------------------------------------
// MUST TRIGGER, no quick fix: the rewrite would not compile or would change meaning.
// ---------------------------------------------------------------------------

// MUST TRIGGER: the receiver reads the thunk's `signal`, which is out of scope once
// the receiver moves to Effect.forEach's first argument; no quick fix.
export const receiverUsesSignal = Effect.promise((signal) => Promise.all(idsFor(signal).map((id) => fetchUser(id))))

// MUST TRIGGER: the callback parameter has the thunk parameter's name; the redeclared
// thunk parameter would shadow it in the moved body; no quick fix.
export const callbackShadowsSignal = Effect.promise((signal) => Promise.all(ids.map((signal) => fetchUser(signal))))

// MUST TRIGGER: a destructured callback parameter next to the thunk's `signal`; the
// destructured names do not collide with it.
// Quick fix: Effect.forEach(entries, ({ id }) => Effect.promise((signal) => fetchUserWithSignal(id, signal)), { concurrency: "unbounded" })
export const destructuredWithSignal = Effect.promise((signal) =>
  Promise.all(entries.map(({ id }) => fetchUserWithSignal(id, signal)))
)

// MUST TRIGGER: a function expression callback reading its own `arguments`; an arrow
// would read another function's `arguments`; no quick fix.
export const callbackUsesArguments = Effect.promise(() =>
  Promise.all(ids.map(function(id) {
    return fetchUser(`${arguments.length}:${id}`)
  }))
)

// MUST TRIGGER: a named function expression callback that calls itself; its name is
// not in scope in the per-item thunk; no quick fix.
export const callbackUsesOwnName = Effect.promise(() =>
  Promise.all(ids.map(function load(id): Promise<User> {
    return id.length > 0 ? fetchUser(id) : load("root")
  }))
)

// MUST TRIGGER: a rest-parameter callback; Effect.forEach passes only (item, index); no quick fix.
export const restParameterCallback = Effect.promise(() => Promise.all(ids.map((...args) => fetchUser(args[0]))))

// MUST TRIGGER: the `.map` receiver is a tuple, not an array or readonly array; no quick fix.
export const tupleReceiver = Effect.promise(() => Promise.all(pairIds.map((id) => fetchUser(id))))

// MUST TRIGGER: the callback returns a plain value; Promise.all accepts it, but the
// per-item Effect.promise thunk must return a promise; no quick fix.
export const plainValueCallback = Effect.promise(() => Promise.all(ids.map((id) => id.length)))

// MUST TRIGGER: the callback returns a value or a promise; no quick fix.
export const valueOrPromiseCallback = Effect.promise(() =>
  Promise.all(ids.map((id) => id.length > 0 ? fetchUser(id) : cachedUser))
)

// MUST TRIGGER: a callback parameter named like the constructor's module would capture
// the copied `Effect.promise` reference; no quick fix.
export const callbackShadowsEffect = Effect.promise(() => Promise.all(ids.map((Effect) => fetchUser(Effect))))

// MUST TRIGGER: a callback parameter initializer reads the thunk's `signal`, which is out
// of scope once the parameters move to Effect.forEach's callback; no quick fix.
export const parameterInitializerUsesSignal = Effect.promise((signal) =>
  Promise.all(ids.map((id: string = String(signal.reason)) => fetchUser(id)))
)

// MUST TRIGGER: the callback body declares the thunk parameter's name; it would collide
// with the redeclared per-item `signal`; no quick fix.
export const bodyDeclaresSignal = Effect.promise((signal) =>
  Promise.all(ids.map((id) => {
    const signal = id.trim()
    return fetchUser(signal)
  }))
)

// MUST TRIGGER: a hoisted `var` in a nested block of the callback body has the same
// collision; no quick fix.
export const bodyVarDeclaresSignal = Effect.promise((signal) =>
  Promise.all(ids.map((id) => {
    if (id.length > 0) {
      var signal: string | undefined = id
    }
    return fetchUser(signal ?? id)
  }))
)

// MUST TRIGGER: a function expression callback declaring a `this` parameter, which an
// arrow cannot declare; no quick fix.
export const thisParameterCallback = Effect.promise(() =>
  Promise.all(ids.map(function(this: void, id) {
    return fetchUser(id)
  }))
)

// MUST TRIGGER: a named function expression callback whose parameter initializer reads
// its own name; the copied parameter list has no such name in scope; no quick fix.
export const parameterInitializerUsesOwnName = Effect.promise(() =>
  Promise.all(maybeIds.map(function load(id = load.name) {
    return fetchUser(id)
  }))
)

// MUST TRIGGER: a function expression callback whose parameter initializer reads its own
// `arguments`; an arrow would read another function's `arguments`; no quick fix.
export const parameterInitializerUsesArguments = Effect.promise(() =>
  Promise.all(maybeIds.map(function(id = String(arguments.length)) {
    return fetchUser(id)
  }))
)

// MUST TRIGGER: a type parameter constrained to an array is a variable-length input.
export const constrainedGeneric = <Tasks extends ReadonlyArray<Promise<User>>>(tasks: Tasks) =>
  Effect.promise(() => Promise.all(tasks))

// MUST NOT TRIGGER: a type parameter constrained to a fixed-length tuple.
export const constrainedFixedTuple = <Tasks extends readonly [Promise<User>, Promise<User>]>(tasks: Tasks) =>
  Effect.promise(() => Promise.all(tasks))

type Signal = AbortSignal

// MUST TRIGGER: the thunk parameter's type annotation names a type that the callback
// body declares again; copied under the callback it would mean that type; no quick fix.
export const thunkParameterTypeCaptured = Effect.promise((signal: Signal) =>
  Promise.all(ids.map((id) => {
    type Signal = string
    const label: Signal = id
    return fetchUserWithSignal(label, signal)
  }))
)

declare const brandedTasks: Array<Promise<User>> & { readonly batchId: string }
declare const brandedPair: readonly [Promise<User>, Promise<User>] & { readonly batchId: string }
declare const arrayAndPair: ReadonlyArray<Promise<User>> & readonly [Promise<User>, Promise<User>]

// MUST TRIGGER: a branded mutable array is still a variable-length array.
export const brandedArray = Effect.promise(() => Promise.all(brandedTasks))

// MUST NOT TRIGGER: a branded fixed-length tuple is still a fixed fan-out.
export const brandedFixedTuple = Effect.promise(() => Promise.all(brandedPair))

// MUST NOT TRIGGER: an array type intersected with a fixed-length tuple has the tuple's length.
export const arrayIntersectedWithPair = Effect.promise(() => Promise.allSettled(arrayAndPair))

declare const pending: ReadonlyArray<Promise<User>>
declare const InjectedPromise: PromiseConstructor
declare const deps: { readonly Promise: PromiseConstructor }
const unboundAll = Promise.all

// MUST TRIGGER: `globalThis.Promise` is the global Promise value.
export const viaGlobalThis = Effect.promise(() => globalThis.Promise.all(pending))

// MUST NOT TRIGGER: a declared value typed PromiseConstructor is not the global Promise.
export const injectedConstructor = Effect.promise(() => InjectedPromise.all(pending))

// MUST NOT TRIGGER: a constructor reached through an object property is not the global Promise.
export const constructorProperty = Effect.promise(() => deps.Promise.allSettled(pending))

// MUST NOT TRIGGER: an unbound alias of the method is not called on the global Promise.
export const unboundMethodAlias = Effect.promise(() => unboundAll(pending))
