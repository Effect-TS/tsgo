import * as SqliteClient from "@effect/sql-sqlite-node/SqliteClient"
import * as Effect from "effect/Effect"
import * as Migrator from "effect/sql/Migrator"
import * as SqlClient from "effect/sql/SqlClient"
import schema from "./_schema.json" with { type: "json" }
import { migrations } from "./migrations.ts"
import { tagged } from "./tagged.ts"

class Database extends SqlClient.Service<Database>()("tagged-template/Database", { schema }) {}

const result = await Effect.gen(function*() {
  yield* Migrator.make({})({ loader: migrations })
  const sql = tagged(yield* Database)
  const minimumAge = 18
  const limit = 2
  const statement = sql`
    SELECT u.id AS userId, u.name, u.email, p.bio AS profileBio
    FROM users AS u LEFT JOIN profiles AS p ON u.id = p.user_id
    WHERE u.age >= ${minimumAge}
    ORDER BY u.name
    LIMIT ${limit}
  `
  const users = yield* statement
  // Inferred: readonly { readonly userId: number; readonly name: string;
  //   readonly email: string | null; readonly profileBio: string | null }[]
  // profiles.bio is NOT NULL, but LEFT JOIN makes it nullable here.
  const cards = users.map((user) => ({
    id: user.userId.toFixed(0),
    name: user.name.toUpperCase(),
    email: user.email?.toLowerCase() ?? "(no email)",
    profile: user.profileBio ?? "(no profile)"
  }))

  const untrustedName = "' OR 1=1 --"
  const matches = yield* sql`SELECT id FROM users WHERE name = ${untrustedName}`

  // Uncomment to see diagnostics on the real tagged-template syntax:
  // sql`SELECT nickname FROM users`
  // sql`SELECT id FROM users WHERE age >= ${"eighteen"}`
  // sql`SELECT ${"name"} FROM users`
  // users[0].email.toLowerCase()
  // users[0].profileBio.toUpperCase()

  const [text, parameters] = statement.compile()
  return {
    sql: text.replace(/\s+/g, " ").trim(),
    parameters,
    cards,
    untrustedNameMatches: matches.length
  }
}).pipe(
  Effect.provide(Database.layer),
  Effect.provide(SqliteClient.layer({ filename: ":memory:" })),
  Effect.runPromise
)

console.log(JSON.stringify(result, null, 2))
