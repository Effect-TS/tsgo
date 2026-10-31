// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics scopedImmediateScopeToScopedWith:warning
import * as Context from "effect/Context"
import * as Effect from "effect/Effect"
import * as Fx from "effect/Effect"
import { gen, scope as currentScope, scoped } from "effect/Effect"
import * as ExecutionStrategy from "effect/ExecutionStrategy"
import { pipe } from "effect/Function"
import * as Layer from "effect/Layer"
import * as Scope from "effect/Scope"
import * as Stream from "effect/Stream"

class Database extends Context.Tag("Database")<Database, {
  readonly query: (sql: string) => Effect.Effect<string>
}>() {}

declare const openConnection: (scope: Scope.Scope) => Effect.Effect<string>
declare const openScopedConnection: (scope: Scope.Scope) => Effect.Effect<string, never, Scope.Scope>
declare const acquire: Effect.Effect<string>
declare const makeCallback: (label: string) => (scope: Scope.Scope) => Effect.Effect<string>
declare const release: (value: string) => Effect.Effect<void>
declare const log: (message: string) => void

// MUST TRIGGER: issue example, .pipe(Effect.scoped) suffix form
// -> Effect.scopedWith((scope) => Effect.gen(function*() { <rest of the body> }))
export const issueExample = Effect.gen(function*() {
  const scope = yield* Effect.scope
  const connection = yield* openConnection(scope)
  yield* Scope.addFinalizer(scope, Effect.log("closing"))
  return connection
}).pipe(Effect.scoped)

// MUST TRIGGER: direct call form Effect.scoped(Effect.gen(...))
// -> Effect.scopedWith((scope) => Effect.gen(function*() { return yield* openConnection(scope) }))
export const directCall = Effect.scoped(Effect.gen(function*() {
  const scope = yield* Effect.scope
  return yield* openConnection(scope)
}))

// MUST TRIGGER: Function.pipe form pipe(Effect.gen(...), Effect.scoped)
// -> Effect.scopedWith((scope) => Effect.gen(...))
export const functionPipe = pipe(
  Effect.gen(function*() {
    const scope = yield* Effect.scope
    return yield* openConnection(scope)
  }),
  Effect.scoped
)

// MUST TRIGGER: the steps after Effect.scoped are kept
// -> Effect.scopedWith((scope) => Effect.gen(...)).pipe(Effect.map((s) => s.length))
export const stepsAfterScoped = Effect.gen(function*() {
  const scope = yield* Effect.scope
  return yield* openConnection(scope)
}).pipe(Effect.scoped, Effect.map((s) => s.length))

// MUST TRIGGER: the binding name is carried over as the callback parameter
// -> Effect.scopedWith((connectionScope) => Effect.gen(...))
export const customBindingName = Effect.gen(function*() {
  const connectionScope = yield* Effect.scope
  yield* Scope.addFinalizer(connectionScope, Effect.log("closing"))
  return yield* openConnection(connectionScope)
}).pipe(Effect.scoped)

// MUST TRIGGER: the body uses services other than Scope; they stay in the requirements
// -> Effect.scopedWith((scope) => Effect.gen(...))
export const otherRequirements = Effect.gen(function*() {
  const scope = yield* Effect.scope
  const db = yield* Database
  yield* Scope.addFinalizer(scope, Effect.log("closing"))
  return yield* db.query("select 1")
}).pipe(Effect.scoped)

// MUST TRIGGER: the retrieved scope is never used again; scopedWith is still equivalent
// -> Effect.scopedWith((scope) => Effect.gen(function*() { return 1 }))
export const unusedScope = Effect.gen(function*() {
  const scope = yield* Effect.scope
  return 1
}).pipe(Effect.scoped)

// MUST TRIGGER: the explicit scope is forwarded to Scope APIs that take it as an argument
// -> Effect.scopedWith((scope) => Effect.gen(...))
export const explicitScopeApis = Effect.gen(function*() {
  const scope = yield* Effect.scope
  const child = yield* Scope.fork(scope, ExecutionStrategy.sequential)
  yield* Scope.addFinalizer(child, Effect.log("child closed"))
  return yield* Scope.extend(acquire, child)
}).pipe(Effect.scoped)

// MUST TRIGGER: namespace import of effect/Effect keeps its qualifier
// -> Fx.scopedWith((scope) => Fx.gen(...))
export const namespaceImport = Fx.gen(function*() {
  const scope = yield* Fx.scope
  return yield* openConnection(scope)
}).pipe(Fx.scoped)

// MUST TRIGGER: named (and renamed) imports resolve to the same APIs
// (the fix falls back to the file's Effect module binding -> Effect.scopedWith((scope) => gen(...)))
export const namedImports = scoped(gen(function*() {
  const scope = yield* currentScope
  return yield* openConnection(scope)
}))

// MUST TRIGGER: combinator spelling Effect.scoped(Effect.flatMap(Effect.scope, f))
// -> Effect.scopedWith((scope) => openConnection(scope))
export const flatMapCall = Effect.scoped(Effect.flatMap(Effect.scope, (scope) => openConnection(scope)))

// MUST TRIGGER: point-free flatMap callback -> Effect.scopedWith(openConnection)
export const flatMapPointFree = Effect.scoped(Effect.flatMap(Effect.scope, openConnection))

// MUST TRIGGER: combinator spelling with a .pipe step
// -> Effect.scopedWith((scope) => Scope.addFinalizer(scope, Effect.log("closing")))
export const flatMapMethodPipe = Effect.scoped(
  Effect.scope.pipe(Effect.flatMap((scope) => Scope.addFinalizer(scope, Effect.log("closing"))))
)

// MUST TRIGGER: combinator spelling as a single Function.pipe flow -> Effect.scopedWith(openConnection)
export const flatMapFunctionPipe = pipe(Effect.scope, Effect.flatMap(openConnection), Effect.scoped)

// MUST TRIGGER: combinator spelling with a .pipe(..., Effect.scoped) flow -> Effect.scopedWith(openConnection)
export const flatMapMethodPipeScoped = Effect.scope.pipe(Effect.flatMap(openConnection), Effect.scoped)

// MUST TRIGGER: v3 Effect.scopeWith(f) is Effect.flatMap(Effect.scope, f) -> Effect.scopedWith(openConnection)
export const scopeWithCall = Effect.scoped(Effect.scopeWith(openConnection))
export const scopeWithPipe = Effect.scopeWith((scope) => openConnection(scope)).pipe(Effect.scoped)

// MUST TRIGGER: the scopeWith callback is the previous step's result -> pipe(makeCallback("db"), Effect.scopedWith)
export const scopeWithPipeStep = pipe(makeCallback("db"), Effect.scopeWith, Effect.scoped)
export const scopeWithPipeStepScopedCall = Effect.scoped(pipe(makeCallback("db"), Effect.scopeWith))
// MUST TRIGGER (no fix): collapsing would call .pipe on the callback
export const scopeWithPipeStepScopedPipeable = pipe(makeCallback("db"), Effect.scopeWith).pipe(Effect.scoped)

// MUST TRIGGER: inside generators and pipelines, only the Effect.scoped-wrapped gen
export const nestedInGen = Effect.gen(function*() {
  return yield* Effect.gen(function*() {
    const scope = yield* Effect.scope
    return yield* openConnection(scope)
  }).pipe(Effect.scoped)
})
export const nestedInFlatMap = Effect.succeed(1).pipe(
  Effect.flatMap(() =>
    Effect.gen(function*() {
      const scope = yield* Effect.scope
      return yield* openConnection(scope)
    }).pipe(Effect.scoped)
  )
)

// MUST NOT TRIGGER: already Effect.scopedWith
export const alreadyScopedWith = Effect.scopedWith((scope) =>
  Effect.gen(function*() {
    const connection = yield* openConnection(scope)
    yield* Scope.addFinalizer(scope, Effect.log("closing"))
    return connection
  })
)

// MUST NOT TRIGGER: the retrieval is not the first statement of the body
export const notFirstStatement = Effect.gen(function*() {
  yield* Effect.log("opening")
  const scope = yield* Effect.scope
  return yield* openConnection(scope)
}).pipe(Effect.scoped)
export const sideEffectFirst = Effect.gen(function*() {
  log("opening")
  const scope = yield* Effect.scope
  return yield* openConnection(scope)
}).pipe(Effect.scoped)

// MUST NOT TRIGGER: the scope is retrieved a second time later in the body (not a single retrieval)
export const retrievedTwice = Effect.gen(function*() {
  const scope = yield* Effect.scope
  const again = yield* Effect.scope
  yield* Scope.addFinalizer(again, Effect.log("closing"))
  return yield* openConnection(scope)
}).pipe(Effect.scoped)

// MUST NOT TRIGGER: Effect.scope is referenced again in a nested callback of the body
export const referencedInCallback = Effect.gen(function*() {
  const scope = yield* Effect.scope
  const later = () => Effect.scope
  log(typeof later)
  return yield* openConnection(scope)
}).pipe(Effect.scoped)

// MUST NOT TRIGGER: other effects in the body still read the ambient Scope from the environment
// (acquireRelease, addFinalizer, scoped helpers); Effect.scopedWith does not provide a Scope,
// so the rewrite would leave Scope in the requirements
export const usesAcquireRelease = Effect.gen(function*() {
  const scope = yield* Effect.scope
  const resource = yield* Effect.acquireRelease(acquire, release)
  yield* Scope.addFinalizer(scope, Effect.log("closing"))
  return resource
}).pipe(Effect.scoped)
export const usesAddFinalizer = Effect.gen(function*() {
  const scope = yield* Effect.scope
  yield* Effect.addFinalizer(() => Effect.log("closing"))
  return yield* openConnection(scope)
}).pipe(Effect.scoped)
export const usesScopedHelper = Effect.gen(function*() {
  const scope = yield* Effect.scope
  return yield* openScopedConnection(scope)
}).pipe(Effect.scoped)
export const usesNestedScopedGen = Effect.gen(function*() {
  const scope = yield* Effect.scope
  yield* Effect.gen(function*() {
    yield* Effect.addFinalizer(() => Effect.log("inner"))
  })
  return yield* openConnection(scope)
}).pipe(Effect.scoped)
export const flatMapCallbackNeedsScope = Effect.scoped(Effect.flatMap(Effect.scope, openScopedConnection))
export const scopeWithCallbackNeedsScope = Effect.scoped(Effect.scopeWith(openScopedConnection))

// MUST NOT TRIGGER: let binding (the retrieval is not a plain const binding)
export const letBinding = Effect.gen(function*() {
  let scope = yield* Effect.scope
  scope = yield* Scope.fork(scope, ExecutionStrategy.sequential)
  return yield* openConnection(scope)
}).pipe(Effect.scoped)

// MUST NOT TRIGGER: the retrieval is not bound to a single identifier
export const discardedRetrieval = Effect.gen(function*() {
  yield* Effect.scope
  return yield* acquire
}).pipe(Effect.scoped)
export const multipleDeclarators = Effect.gen(function*() {
  const scope = yield* Effect.scope, label = "db"
  return yield* openConnection(scope).pipe(Effect.map((s) => `${label}:${s}`))
}).pipe(Effect.scoped)

// MUST NOT TRIGGER: the scope is retrieved through the Scope tag, not Effect.scope
export const tagRetrieval = Effect.gen(function*() {
  const scope = yield* Scope.Scope
  return yield* openConnection(scope)
}).pipe(Effect.scoped)

// MUST NOT TRIGGER: Effect.scoped does not wrap the gen directly (a transformation sits in between)
export const transformedBeforeScoped = Effect.gen(function*() {
  const scope = yield* Effect.scope
  return yield* openConnection(scope)
}).pipe(Effect.map((s) => s.length), Effect.scoped)

// MUST NOT TRIGGER: the retrieval is not in a body wrapped by Effect.scoped;
// the Scope comes from an outer provider and scopedWith would introduce a fresh one
export const unwrapped = Effect.gen(function*() {
  const scope = yield* Effect.scope
  return yield* openConnection(scope)
})
export const unwrappedScopeWith = Effect.scopeWith(openConnection)
export const layerScoped = Layer.scoped(
  Database,
  Effect.gen(function*() {
    const scope = yield* Effect.scope
    yield* Scope.addFinalizer(scope, Effect.log("closing"))
    return { query: (sql: string) => Effect.succeed(sql) }
  })
)
export const streamScoped = Stream.scoped(Effect.gen(function*() {
  const scope = yield* Effect.scope
  return yield* openConnection(scope)
}))
export const acquireBody = Effect.acquireRelease(
  Effect.gen(function*() {
    const scope = yield* Effect.scope
    return yield* openConnection(scope)
  }),
  release
)
export const fnUntracedBody = Effect.fnUntraced(function*() {
  const scope = yield* Effect.scope
  return yield* openConnection(scope)
})

// MUST NOT TRIGGER: Effect.scoped as an Effect.fn post-transformation; the generator is the
// function body, so there is no Effect.gen expression to hand to Effect.scopedWith
export const fnWithScoped = Effect.fn("fnWithScoped")(function*() {
  const scope = yield* Effect.scope
  return yield* openConnection(scope)
}, Effect.scoped)
export const fnUntracedWithScoped = Effect.fnUntraced(function*() {
  const scope = yield* Effect.scope
  return yield* openConnection(scope)
}, Effect.scoped)

// MUST NOT TRIGGER: Effect.scoped wrapping something that is neither a gen nor a flatMap of Effect.scope
export const scopedScope = Effect.scoped(Effect.scope)
export const scopedAcquire = Effect.scoped(Effect.acquireRelease(acquire, release))
export const flatMapOtherEffect = Effect.scoped(Effect.flatMap(acquire, () => Effect.scope))
export const mapScope = Effect.scoped(Effect.map(Effect.scope, () => 1))
export const andThenScope = Effect.scoped(Effect.andThen(Effect.scope, openConnection))

// MUST NOT TRIGGER: user-defined APIs with the same names
const Local = {
  scoped: <A, E, R>(self: Effect.Effect<A, E, R>) => Effect.scoped(self),
  scope: Effect.succeed("not a scope")
}
export const localScoped = Local.scoped(Effect.gen(function*() {
  const scope = yield* Effect.scope
  return yield* openConnection(scope)
}))
export const localScope = Effect.gen(function*() {
  const label = yield* Local.scope
  return label
}).pipe(Effect.scoped)
