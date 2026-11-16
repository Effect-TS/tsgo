import { Context, Effect } from "effect"

// The quick fix rewrites this to Effect.provideServiceEffect(program, Database, makeConnection).
// Notes:
// - After the fix, `program` is evaluated once when the expression is built, not on every run of the callback.
// - The Effect.gen form (`const conn = yield* makeConnection; return yield* Effect.provideService(program, Database, conn)`)
//   is deliberately not reported.
// - On Effect v3, data-first `Effect.andThen(acquire, callback)` is not reported, since its overloads do not form a dual.

interface DatabaseConnection {
  readonly query: (sql: string) => Effect.Effect<string>
}

const Database = Context.Service<DatabaseConnection>("Database")

declare const makeConnection: Effect.Effect<DatabaseConnection>

const program = Effect.gen(function*() {
  const db = yield* Effect.service(Database)
  return yield* db.query("SELECT * FROM users")
})

export const runWithDatabase = makeConnection.pipe(
  Effect.flatMap((conn) => Effect.provideService(program, Database, conn))
)
