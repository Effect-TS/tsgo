import { Context, Effect, pipe } from "effect"
import * as E from "effect/Effect"
import { flatMap as bind, provideService as provide } from "effect/Effect"

interface DatabaseConnection {
  readonly query: (sql: string) => Effect.Effect<string>
}

const Database = Context.Service<DatabaseConnection>("Database")
class Clock extends Context.Service<Clock, { readonly now: Effect.Effect<number> }>()("Clock") {}
const CurrentMeta = Context.Reference<{ readonly attempt: number }>("CurrentMeta", {
  defaultValue: () => ({ attempt: 0 })
})

declare const makeConnection: Effect.Effect<DatabaseConnection, "acquire", "env">
declare const makeClock: Effect.Effect<Clock["Service"]>
declare const makeMeta: Effect.Effect<{ readonly attempt: number }>
declare const other: DatabaseConnection
declare const keyFor: (conn: DatabaseConnection) => typeof Database
declare const useConnection: (conn: DatabaseConnection) => Effect.Effect<string, never, DatabaseConnection>
declare const step: (n: number) => Effect.Effect<{ readonly attempt: number }>
declare const makeNumber: Effect.Effect<number>

const program = Effect.gen(function*() {
  const db = yield* Effect.service(Database)
  return yield* db.query("SELECT * FROM users")
})
const clockProgram = Effect.gen(function*() {
  const clock = yield* Clock
  return yield* clock.now
})
const metaProgram = Effect.gen(function*() {
  const meta = yield* CurrentMeta
  return meta.attempt
})

// MUST TRIGGER: the issue's example, pipeable flatMap with a data-first provideService.
// Fix: Effect.provideServiceEffect(program, Database, makeConnection)
export const pipeableDataFirst = makeConnection.pipe(
  Effect.flatMap((conn) => Effect.provideService(program, Database, conn))
)

// MUST TRIGGER: data-first flatMap with a pipeable provideService.
// Fix: program.pipe(Effect.provideServiceEffect(Database, makeConnection)) or the data-first form
export const dataFirstPipeable = Effect.flatMap(makeConnection, (conn) => program.pipe(Effect.provideService(Database, conn)))

// MUST TRIGGER: function pipe on both sides.
export const functionPipe = pipe(makeConnection, Effect.flatMap((conn) => pipe(program, Effect.provideService(Database, conn))))

// MUST TRIGGER: data-first flatMap with a data-first provideService.
export const dataFirstDataFirst = Effect.flatMap(makeConnection, (conn) => Effect.provideService(program, Database, conn))

// MUST TRIGGER: curried flatMap applied to the acquisition.
export const curriedFlatMap = Effect.flatMap((conn: DatabaseConnection) => Effect.provideService(program, Database, conn))(
  makeConnection
)

// MUST TRIGGER: curried provideService applied to the program (normalizes to the same flow step).
export const curriedProvideService = makeConnection.pipe(
  Effect.flatMap((conn) => Effect.provideService(Database, conn)(program))
)

// MUST TRIGGER: Effect.andThen given a callback, in all three spellings.
export const andThenPipeable = makeConnection.pipe(
  Effect.andThen((conn) => Effect.provideService(program, Database, conn))
)
export const andThenDataFirst = Effect.andThen(makeConnection, (conn) => program.pipe(Effect.provideService(Database, conn)))
export const andThenFunctionPipe = pipe(
  makeConnection,
  Effect.andThen((conn) => pipe(program, Effect.provideService(Database, conn)))
)

// MUST TRIGGER: a block body whose only statement returns the provideService call.
export const blockBody = makeConnection.pipe(Effect.flatMap((conn) => {
  return Effect.provideService(program, Database, conn)
}))

// MUST TRIGGER: a function expression callback.
export const functionExpression = makeConnection.pipe(Effect.flatMap(function(conn) {
  return program.pipe(Effect.provideService(Database, conn))
}))

// MUST TRIGGER: an annotated parameter is still a plain single parameter.
export const annotatedParameter = makeConnection.pipe(
  Effect.flatMap((conn: DatabaseConnection) => Effect.provideService(program, Database, conn))
)

// MUST TRIGGER: a class-based service key.
export const classKey = makeClock.pipe(Effect.flatMap((clock) => Effect.provideService(clockProgram, Clock, clock)))

// MUST TRIGGER: a Context.Reference key (the real-world Stream.fromEffectSchedule site).
// The flatMap is the second step of the outer flow and another step follows it.
// Fix: Effect.provideServiceEffect(metaProgram, CurrentMeta, Effect.suspend(() => step(1))).pipe(Effect.map(...))
export const realWorldSite = Effect.suspend(() => step(1)).pipe(
  Effect.flatMap((meta) => Effect.provideService(metaProgram, CurrentMeta, meta)),
  Effect.map((attempt) => attempt + 1)
)

// MUST TRIGGER: the acquisition is itself a pipe prefix; the fix must keep the earlier steps in the acquisition.
// Fix: Effect.provideServiceEffect(metaProgram, CurrentMeta, makeMeta.pipe(Effect.map(...)))
export const acquisitionPrefix = makeMeta.pipe(
  Effect.map((meta) => ({ attempt: meta.attempt + 1 })),
  Effect.flatMap((meta) => Effect.provideService(metaProgram, CurrentMeta, meta))
)

// MUST TRIGGER: a step after the flatMap is kept after the rewritten expression.
export const laterOuterStep = makeConnection.pipe(
  Effect.flatMap((conn) => Effect.provideService(program, Database, conn)),
  Effect.map((rows) => rows.length)
)

// MUST TRIGGER: steps before provideService belong to `self`, which still does not mention the parameter.
// Fix: Effect.provideServiceEffect(program.pipe(Effect.map(...)), Database, makeConnection)
// (or program.pipe(Effect.map(...), Effect.provideServiceEffect(Database, makeConnection)))
export const selfWithSteps = makeConnection.pipe(
  Effect.flatMap((conn) => program.pipe(Effect.map((rows) => rows.length), Effect.provideService(Database, conn)))
)
export const selfDataFirstNested = makeConnection.pipe(
  Effect.flatMap((conn) => Effect.provideService(Effect.map(program, (rows) => rows.length), Database, conn))
)

// MUST TRIGGER: an inner callback in `self` declares its own `conn`, a different binding from the parameter.
export const shadowedInSelf = makeConnection.pipe(
  Effect.flatMap((conn) =>
    Effect.provideService(Effect.map(Effect.succeed(other), (conn) => conn.query), Database, conn)
  )
)

// MUST TRIGGER: module, namespace and imported aliases.
export const namespaceAlias = makeConnection.pipe(E.flatMap((conn) => E.provideService(program, Database, conn)))
export const importedAlias = makeConnection.pipe(bind((conn) => provide(program, Database, conn)))

// MUST TRIGGER: inside an Effect.gen body.
export const insideGen = Effect.gen(function*() {
  yield* Effect.log("connecting")
  return yield* makeConnection.pipe(Effect.flatMap((conn) => Effect.provideService(program, Database, conn)))
})

// MUST TRIGGER: inside an Effect.fn body.
export const insideFn = Effect.fn("insideFn")(function*() {
  return yield* makeConnection.pipe(Effect.flatMap((conn) => Effect.provideService(program, Database, conn)))
})

// MUST NOT TRIGGER: already uses provideServiceEffect.
export const alreadyGood = Effect.provideServiceEffect(program, Database, makeConnection)
export const alreadyGoodPipeable = program.pipe(Effect.provideServiceEffect(Database, makeConnection))

// MUST NOT TRIGGER: the Effect.gen form is deliberately out of scope.
export const genForm = Effect.gen(function*() {
  const conn = yield* makeConnection
  return yield* Effect.provideService(program, Database, conn)
})

// MUST NOT TRIGGER: the program (`self`) mentions the parameter.
export const selfUsesParameter = makeConnection.pipe(
  Effect.flatMap((conn) => Effect.provideService(useConnection(conn), Database, conn))
)
export const selfPipeUsesParameter = makeConnection.pipe(
  Effect.flatMap((conn) => conn.query("SELECT 1").pipe(Effect.provideService(Database, conn)))
)

// MUST NOT TRIGGER: the parameter is used in an earlier step of `self`, not in its receiver;
// the "earlier steps belong to self" logic must check those steps too.
export const selfStepUsesParameter = makeConnection.pipe(
  Effect.flatMap((conn) => program.pipe(Effect.map(() => conn), Effect.provideService(Database, conn)))
)
export const selfDataFirstStepUsesParameter = makeConnection.pipe(
  Effect.flatMap((conn) => Effect.provideService(Effect.map(program, () => conn), Database, conn))
)

// MUST NOT TRIGGER: the key mentions the parameter.
export const keyUsesParameter = makeConnection.pipe(
  Effect.flatMap((conn) => Effect.provideService(program, keyFor(conn), conn))
)

// MUST NOT TRIGGER: the implementation is not the bare parameter.
export const implementationObject = makeConnection.pipe(
  Effect.flatMap((conn) => Effect.provideService(program, Database, { query: conn.query }))
)
export const implementationSpread = makeConnection.pipe(
  Effect.flatMap((conn) => Effect.provideService(program, Database, { ...conn }))
)
export const implementationAssertion = makeConnection.pipe(
  Effect.flatMap((conn) => Effect.provideService(program, Database, conn as DatabaseConnection))
)
export const implementationOther = makeConnection.pipe(
  Effect.flatMap((conn) => Effect.provideService(program, Database, other))
)

// MUST NOT TRIGGER: the acquisition's success type (number) is not assignable to the service shape of the key;
// the `any` annotation hides the mismatch. Rewriting to Effect.provideServiceEffect(program, Database, makeNumber)
// would not type-check, so the rule must check that the acquisition's success type is assignable to the key's
// service type (issue #644, proposed behavior bullet 3).
export const anyParameter = makeNumber.pipe(
  Effect.flatMap((conn: any) => Effect.provideService(program, Database, conn))
)

// MUST NOT TRIGGER: a step after provideService inside the callback.
export const innerLaterStep = makeConnection.pipe(
  Effect.flatMap((conn) => program.pipe(Effect.provideService(Database, conn), Effect.map((rows) => rows.length)))
)
export const innerLaterStepDataFirst = makeConnection.pipe(
  Effect.flatMap((conn) => Effect.map(Effect.provideService(program, Database, conn), (rows) => rows.length))
)

// MUST NOT TRIGGER: the callback does more than return the provideService call.
export const extraStatements = makeConnection.pipe(Effect.flatMap((conn) => {
  console.log(conn)
  return Effect.provideService(program, Database, conn)
}))
export const conditionalReturn = makeConnection.pipe(
  Effect.flatMap((conn) => (conn === other ? program.pipe(Effect.provideService(Database, other)) : Effect.provideService(program, Database, conn)))
)

// MUST NOT TRIGGER: the parameter is not a plain single identifier.
export const destructuredParameter = makeConnection.pipe(
  Effect.flatMap(({ query }) => Effect.provideService(program, Database, { query }))
)
export const defaultedParameter = makeConnection.pipe(
  Effect.flatMap((conn = other) => Effect.provideService(program, Database, conn))
)
export const restParameter = makeConnection.pipe(
  Effect.flatMap((...conns) => Effect.provideService(program, Database, conns[0]))
)

// MUST NOT TRIGGER: andThen given a thunk or an effect instead of a parameter callback.
export const andThenThunk = makeConnection.pipe(Effect.andThen(() => Effect.provideService(program, Database, other)))
export const andThenEffect = makeConnection.pipe(Effect.andThen(Effect.provideService(program, Database, other)))

// MUST NOT TRIGGER: a different provide API inside the callback.
export const provideServiceEffectInside = makeConnection.pipe(
  Effect.flatMap((conn) => Effect.provideServiceEffect(program, Database, Effect.succeed(conn)))
)

// MUST NOT TRIGGER: a plain provideService with no flatMap around it.
export const plainProvideService = Effect.provideService(program, Database, other)

// MUST NOT TRIGGER: unrelated APIs with the same method names.
const custom = {
  flatMap: <A, B>(self: A, f: (a: A) => B): B => f(self),
  provideService: <A>(self: A, _key: unknown, _impl: unknown): A => self
}
export const customApis = custom.flatMap(other, (conn) => custom.provideService(program, Database, conn))
export const customFlatMap = custom.flatMap(other, (conn) => Effect.provideService(program, Database, conn))
export const customProvideService = makeConnection.pipe(
  Effect.flatMap((conn) => custom.provideService(program, Database, conn))
)
