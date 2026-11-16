// @effect-v3
import { Context, Effect } from "effect"

const Database = Context.GenericTag<DatabaseConnection>("Database")
interface DatabaseConnection {
  readonly query: (sql: string) => Effect.Effect<string>
}

declare const makeConnection: Effect.Effect<DatabaseConnection>

const program = Effect.gen(function*() {
  const db = yield* Database
  return yield* db.query("SELECT 1")
})

// MUST NOT TRIGGER: `self` refers to the named function expression itself, which is
// unbound once `self` moves out of the callback.
export const ownName = makeConnection.pipe(Effect.flatMap(function callback(conn) {
  return Effect.provideService(Effect.as(program, callback.name), Database, conn)
}))

// MUST NOT TRIGGER: the callback name is used inside a nested arrow in `self` (the reviewer's example).
export const ownNameInSync = makeConnection.pipe(Effect.flatMap(function callback(conn) {
  return Effect.provideService(Effect.sync(() => callback.name), Database, conn)
}))

// MUST NOT TRIGGER: `new.target`, `arguments` and `this` of a function-expression callback
// change meaning once `self` leaves the callback.
export const newTarget = makeConnection.pipe(Effect.flatMap(function(conn) {
  return Effect.provideService(Effect.as(program, new.target), Database, conn)
}))
export const argumentsObject = makeConnection.pipe(Effect.flatMap(function(conn) {
  return Effect.provideService(Effect.as(program, arguments.length), Database, conn)
}))
export const thisInNestedArrow = makeConnection.pipe(Effect.flatMap(function(this: unknown, conn) {
  return Effect.provideService(Effect.sync(() => this), Database, conn)
}))

// MUST TRIGGER: a named function expression that does not use its own name.
export const unusedName = makeConnection.pipe(Effect.flatMap(function callback(conn) {
  return Effect.provideService(program, Database, conn)
}))

// MUST TRIGGER: `self` declares its own bindings, which move along with it.
export const ownInnerFunction = makeConnection.pipe(Effect.flatMap(function(conn) {
  return Effect.provideService(Effect.flatMap(Effect.void, function inner() { return Effect.as(program, inner.name) }), Database, conn)
}))

// MUST TRIGGER: an arrow callback does not bind `arguments` or `new.target`; they belong to
// the enclosing function, which `self` stays inside.
export function enclosing(this: unknown, ..._args: ReadonlyArray<unknown>) {
  return makeConnection.pipe(
    Effect.flatMap((conn) => Effect.provideService(Effect.as(program, [arguments.length, new.target, this]), Database, conn))
  )
}

// MUST NOT TRIGGER: computed member names are evaluated in the enclosing scope, so `arguments`
// and `this` there still belong to the function-expression callback (the reviewer's example).
export function computedNameArguments(_label: string) {
  return makeConnection.pipe(Effect.flatMap(function(conn) {
    return Effect.provideService(Effect.as(program, { [String(arguments.length)]() {} }), Database, conn)
  }))
}
export const computedNameThis = makeConnection.pipe(Effect.flatMap(function(this: unknown, conn) {
  return Effect.provideService(Effect.as(program, { [String(this)]: 1 }), Database, conn)
}))
export const newTargetInArrow = makeConnection.pipe(Effect.flatMap(function(conn) {
  return Effect.provideService(Effect.sync(() => new.target), Database, conn)
}))

// MUST TRIGGER: `this` and `arguments` inside class members and method bodies belong to those
// members, not to the callback, so moving `self` out of the callback keeps their meaning.
export const classFieldThis = makeConnection.pipe(Effect.flatMap(function(conn) {
  return Effect.provideService(Effect.as(program, new (class { readonly self = this })()), Database, conn)
}))
export const methodBodyThis = makeConnection.pipe(Effect.flatMap(function(conn) {
  return Effect.provideService(Effect.as(program, { m() { return [this, arguments.length] } }), Database, conn)
}))
export const staticBlockThis = makeConnection.pipe(Effect.flatMap(function(conn) {
  return Effect.provideService(Effect.as(program, class { static {
    void this
  } }), Database, conn)
}))

// MUST NOT TRIGGER: shorthand `{ arguments }` in a function-expression callback refers to the
// callback's own arguments object, which would become the enclosing function's after the fix.
export function shorthandArgumentsInFunction(label: string) {
  return makeConnection.pipe(Effect.flatMap(function(conn) {
    return Effect.provideService(Effect.sync(() => ({ arguments })), Database, conn)
  }), Effect.as(label))
}

// MUST TRIGGER: an arrow callback does not bind `arguments`, so shorthand `{ arguments }` already
// refers to the enclosing function's and the fix keeps it.
export function shorthandArgumentsInArrow(label: string) {
  return makeConnection.pipe(
    Effect.flatMap((conn) => Effect.provideService(Effect.sync(() => ({ arguments })), Database, conn)),
    Effect.as(label)
  )
}
