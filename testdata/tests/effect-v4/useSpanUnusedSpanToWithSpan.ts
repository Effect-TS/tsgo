// @effect-v4
// @effect-diagnostics *:off
// @effect-diagnostics useSpanUnusedSpanToWithSpan:warning
import { Effect } from "effect"
import * as Eff from "effect/Effect"
import { useSpan as namedUseSpan } from "effect/Effect"
import type { Span } from "effect/Tracer"

declare const fetchUser: (id: string) => Effect.Effect<{ name: string }>
declare const withConnectionSpan: <A>(span: Span, effect: Effect.Effect<A>) => Effect.Effect<A>

// MUST TRIGGER: underscore-prefixed parameter, never referenced (issue example).
// fix: fetchUser(id).pipe(Effect.withSpan("fetch-user"))
export const underscoreParam = (id: string) => Effect.useSpan("fetch-user", (_span) => fetchUser(id))

// MUST TRIGGER: bare underscore parameter.
// fix: fetchUser(id).pipe(Effect.withSpan("fetch-user"))
export const bareUnderscore = (id: string) => Effect.useSpan("fetch-user", (_) => fetchUser(id))

// MUST TRIGGER: the callback declares no parameter at all.
// fix: fetchUser(id).pipe(Effect.withSpan("fetch-user"))
export const noParam = (id: string) => Effect.useSpan("fetch-user", () => fetchUser(id))

// MUST TRIGGER: a regular parameter name with zero references in the body.
// fix: fetchUser(id).pipe(Effect.withSpan("fetch-user"))
export const unreferencedParam = (id: string) => Effect.useSpan("fetch-user", (span) => fetchUser(id))

// MUST TRIGGER: explicitly annotated parameter that is never referenced.
// fix: fetchUser(id).pipe(Effect.withSpan("fetch-user"))
export const annotatedParam = (id: string) => Effect.useSpan("fetch-user", (span: Span) => fetchUser(id))

// MUST TRIGGER: three-argument arity, the options are carried over to Effect.withSpan.
// fix: fetchUser(id).pipe(Effect.withSpan("fetch-user", { attributes: { userId: id }, kind: "client" }))
export const withOptions = (id: string) =>
  Effect.useSpan("fetch-user", { attributes: { userId: id }, kind: "client" }, () => fetchUser(id))

// MUST TRIGGER: three-argument arity with an unused named parameter.
// fix: fetchUser(id).pipe(Effect.withSpan("fetch-user", { attributes: { userId: id } }))
export const withOptionsUnusedParam = (id: string) =>
  Effect.useSpan("fetch-user", { attributes: { userId: id } }, (span) => fetchUser(id))

// MUST TRIGGER: the body reads only `const` bindings declared before the call and parameters.
// fix: fetchUser(`${tenant}/${id}`).pipe(Effect.withSpan("fetch-user"))
const tenant = "acme"
export const readsConstsAndParams = (id: string) => Effect.useSpan("fetch-user", () => fetchUser(`${tenant}/${id}`))

// MUST TRIGGER: the body is a conditional expression, so the fix must parenthesize it.
// fix: (preferCache ? fetchUser("cached") : fetchUser(id)).pipe(Effect.withSpan("fetch-user"))
declare const preferCache: boolean
export const conditionalBody = (id: string) =>
  Effect.useSpan("fetch-user", () => preferCache ? fetchUser("cached") : fetchUser(id))

// MUST TRIGGER: block-bodied arrow with a single return statement.
// fix: fetchUser(id).pipe(Effect.withSpan("fetch-user"))
export const blockBody = (id: string) =>
  Effect.useSpan("fetch-user", (_span) => {
    return fetchUser(id)
  })

// MUST TRIGGER, no fix: block-bodied arrow with several statements (there is no single
// expression to move out of the callback).
export const blockBodyStatements = (id: string) =>
  Effect.useSpan("fetch-user", (span) => {
    const normalized = id.trim()
    return fetchUser(normalized)
  })

// MUST TRIGGER: function expression callback with an unused parameter and a single return.
// fix: fetchUser(id).pipe(Effect.withSpan("fetch-user"))
export const functionExpression = (id: string) =>
  Effect.useSpan("fetch-user", function(span) {
    return fetchUser(id)
  })

// MUST TRIGGER: the callback returns an Effect.gen that never touches the span.
// fix: Effect.gen(function*() { ... }).pipe(Effect.withSpan("fetch-user"))
export const genBody = (id: string) =>
  Effect.useSpan("fetch-user", () =>
    Effect.gen(function*() {
      const user = yield* fetchUser(id)
      return user.name
    }))

// MUST TRIGGER: nested inside Effect.gen; children of fetchUser would not nest under "inner".
// fix: fetchUser("a").pipe(Effect.withSpan("inner"))
export const insideGen = Effect.gen(function*() {
  const user = yield* Effect.useSpan("inner", (_span) => fetchUser("a"))
  return user.name
})

// MUST TRIGGER: inside an Effect.fn body.
// fix: fetchUser(id).pipe(Effect.withSpan("inner"))
export const insideFn = Effect.fn("insideFn")(function*(id: string) {
  return yield* Effect.useSpan("inner", () => fetchUser(id))
})

// MUST TRIGGER: the useSpan call is itself piped further; only the useSpan call is replaced.
// fix: fetchUser(id).pipe(Effect.withSpan("fetch-user")).pipe(Effect.map((user) => user.name))
export const pipedFurther = (id: string) =>
  Effect.useSpan("fetch-user", () => fetchUser(id)).pipe(Effect.map((user) => user.name))

// MUST TRIGGER: parenthesized inline callback.
// fix: fetchUser(id).pipe(Effect.withSpan("fetch-user"))
export const parenthesizedCallback = (id: string) => Effect.useSpan("fetch-user", ((_span) => fetchUser(id)))

// MUST TRIGGER: namespace import of effect/Effect under another name; the fix reuses that namespace.
// fix: fetchUser(id).pipe(Eff.withSpan("fetch-user"))
export const namespaceAlias = (id: string) => Eff.useSpan("fetch-user", () => fetchUser(id))

// MUST TRIGGER, no fix: renamed named import of useSpan; there is no namespace receiver to
// reuse for withSpan.
export const namedImportAlias = (id: string) => namedUseSpan("fetch-user", () => fetchUser(id))

// MUST TRIGGER (false-negative trap): the only `span` in the body is a shadowing local in a
// nested scope, so the callback's own parameter has zero references.
// fix: Effect.gen(function*() { ... }).pipe(Effect.withSpan("fetch-user"))
export const shadowedByLocal = (id: string) =>
  Effect.useSpan("fetch-user", (span) =>
    Effect.gen(function*() {
      const span = yield* fetchUser(id)
      return span.name
    }))

// MUST TRIGGER (false-negative trap): the inner callback's parameter shadows the outer span.
// fix: fetchUser(id).pipe(Effect.map((span) => span.name)).pipe(Effect.withSpan("fetch-user"))
export const shadowedByInnerParam = (id: string) =>
  Effect.useSpan("fetch-user", (span) => fetchUser(id).pipe(Effect.map((span) => span.name)))

// MUST TRIGGER (false-negative trap): `.span` is a property name, not a reference to the parameter.
// fix: fetchUser(request.span).pipe(Effect.withSpan("fetch-user"))
declare const request: { readonly span: string }
export const propertyNamedSpan = Effect.useSpan("fetch-user", (span) => fetchUser(request.span))

// MUST TRIGGER, no fix: the body reads a `let` binding outside any nested function; moving the
// body out of the callback would read `counter` once, eagerly, instead of on each run.
let counter = 0
export const readsLet = Effect.useSpan("fetch-user", () => fetchUser(String(counter)))
export const bumpCounter = () => {
  counter++
}

// MUST TRIGGER, no fix: the body reads a `var` binding outside any nested function.
var legacyId = "legacy"
export const readsVar = Effect.useSpan("fetch-user", () => fetchUser(legacyId))

// MUST TRIGGER: the `let` binding is read only inside a nested function of the body, so moving
// the body out of the callback does not change what it observes.
// fix: Effect.sync(() => counter).pipe(Effect.andThen((n) => fetchUser(String(n)))).pipe(Effect.withSpan("fetch-user"))
export const readsLetInsideThunk = Effect.useSpan(
  "fetch-user",
  () => Effect.sync(() => counter).pipe(Effect.andThen((n) => fetchUser(String(n))))
)

// MUST TRIGGER, no fix: the body reads a `const` declared after the useSpan call; evaluating the
// body eagerly would hit the temporal dead zone.
export const readsLaterConst = Effect.useSpan("fetch-user", () => fetchUser(later))
const later = "later-id"

// MUST TRIGGER: the binding declared after the call is read only inside a nested function of
// the body, which still runs only when the effect runs.
// fix: Effect.sync(() => laterInThunk).pipe(Effect.andThen(fetchUser)).pipe(Effect.withSpan("fetch-user"))
export const readsLaterConstInsideThunk = Effect.useSpan(
  "fetch-user",
  () => Effect.sync(() => laterInThunk).pipe(Effect.andThen(fetchUser))
)
const laterInThunk = "later-id"

// MUST TRIGGER (near-miss for the `arguments` exclusion): `arguments` is read only inside a
// nested non-arrow function, which binds its own `arguments` object.
// fix: Effect.sync(function() { return arguments.length }).pipe(Effect.withSpan("fetch-user"))
export const argumentsInNestedFunction = Effect.useSpan("fetch-user", () => Effect.sync(function() { return arguments.length }))

// MUST TRIGGER (near-miss for the `arguments` exclusion): `arguments` is only a property name.
// fix: Effect.succeed({ arguments: 1 }.arguments).pipe(Effect.withSpan("fetch-user"))
export const argumentsPropertyName = Effect.useSpan("fetch-user", () => Effect.succeed({ arguments: 1 }.arguments))

// MUST NOT TRIGGER: the span is used to set an attribute.
export const setsAttribute = (id: string) =>
  Effect.useSpan("fetch-user", (span) =>
    Effect.sync(() => span.attribute("userId", id)).pipe(Effect.andThen(fetchUser(id))))

// MUST NOT TRIGGER: the span is only forwarded to another function (Statement.ts pattern).
export const forwardsSpan = (id: string) =>
  Effect.useSpan("fetch-user", (span) => withConnectionSpan(span, fetchUser(id)))

// MUST NOT TRIGGER: the span is read inside a nested Effect.gen closure.
export const readsInNestedClosure = (id: string) =>
  Effect.useSpan("fetch-user", (span) =>
    Effect.gen(function*() {
      yield* Effect.log(`trace ${span.traceId} span ${span.spanId}`)
      return yield* fetchUser(id)
    }))

// MUST NOT TRIGGER: the span is used in the three-argument arity.
export const usedWithOptions = (id: string) =>
  Effect.useSpan("fetch-user", { attributes: { userId: id } }, (span) => Effect.succeed(span.name))

// MUST NOT TRIGGER: an underscore-prefixed parameter that IS referenced is still a real use.
export const underscoreButUsed = Effect.useSpan("fetch-user", (_span) => Effect.succeed(_span.spanId))

// MUST NOT TRIGGER: the span is consumed through a destructuring pattern.
export const destructured = Effect.useSpan("fetch-user", ({ spanId, traceId }) => Effect.succeed(`${traceId}/${spanId}`))

// MUST NOT TRIGGER: the span is reached through `arguments` in a function expression.
export const viaArguments = Effect.useSpan("fetch-user", function() {
  const span: Span = arguments[0]
  return Effect.succeed(span.name)
})

// MUST TRIGGER: an arrow callback body reads the enclosing function's `arguments`, which cannot
// reach the span and stays the same once the body moves out of the callback.
// fix: fetchUser(`${id}/${arguments.length}`).pipe(Effect.withSpan("fetch-user"))
export const argumentsInArrowCallback = function(id: string) {
  return Effect.useSpan("fetch-user", () => fetchUser(`${id}/${arguments.length}`))
}

// MUST NOT TRIGGER: the callback's own `arguments` is read inside a nested arrow function.
export const argumentsInNestedArrow = Effect.useSpan("fetch-user", function() {
  return Effect.sync(() => arguments.length)
})

// MUST NOT TRIGGER: the callback's own `arguments` is read through a shorthand property.
export const argumentsShorthand = Effect.useSpan("fetch-user", function() {
  return Effect.succeed({ arguments }.arguments.length)
})

// MUST NOT TRIGGER: the callback is not an inline function literal (identifier).
const ignoreSpan = (_span: Span) => fetchUser("a")
export const identifierCallback = Effect.useSpan("fetch-user", ignoreSpan)

// MUST NOT TRIGGER: the callback is produced by a call expression.
declare const makeHandler: () => (span: Span) => Effect.Effect<{ name: string }>
export const callResultCallback = Effect.useSpan("fetch-user", makeHandler())

// MUST NOT TRIGGER: already using Effect.withSpan (both dual forms).
export const alreadyWithSpan = (id: string) => fetchUser(id).pipe(Effect.withSpan("fetch-user"))
export const alreadyWithSpanDataFirst = (id: string) => Effect.withSpan(fetchUser(id), "fetch-user")

// MUST NOT TRIGGER: useSpan referenced but not called.
export const notCalled = Effect.useSpan

// MUST NOT TRIGGER: a local API that merely shares the member name.
const LocalTracer = {
  useSpan: <A>(_name: string, evaluate: (span: string) => A): A => evaluate("local")
}
export const localApi = LocalTracer.useSpan("fetch-user", () => 1)

// MUST NOT TRIGGER: other span APIs with ignored callbacks are out of scope.
export const makeSpanUnused = Effect.makeSpan("fetch-user").pipe(Effect.andThen(() => fetchUser("a")))
