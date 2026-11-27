// @effect-diagnostics *:off
// @effect-diagnostics sqlTaggedRowTypeToSqlSchema:warning
import { Effect, pipe, Schema } from "effect"
import { SqlClient, SqlSchema, Statement } from "effect/sql"

interface CommandRow {
  readonly command_id: string
  readonly fingerprint: string
  readonly fingerprint_version: number
}

type CommandIdRow = Pick<CommandRow, "command_id">

declare const minted: string
declare const keyHash: string

export const mintCommand = Effect.gen(function*() {
  const sql = yield* SqlClient.SqlClient

  // MUST TRIGGER: inline object row type on the sql tag (the reported example)
  const inserted = yield* sql<{ command_id: string }>`
    INSERT INTO commands (command_id, key_hash)
    VALUES (${minted}, ${keyHash})
    ON CONFLICT (key_hash) DO NOTHING
    RETURNING command_id
  `.pipe(Effect.orDie)
  if (inserted.length > 0) return minted

  // MUST TRIGGER: named interface row type, result destructured (the reported example)
  const [existing] = yield* sql<CommandRow>`
    SELECT command_id, fingerprint, fingerprint_version
    FROM commands
    WHERE key_hash = ${keyHash}
  `.pipe(Effect.orDie)
  return existing.command_id
})

export const shapes = Effect.gen(function*() {
  const sql = yield* SqlClient.SqlClient

  // MUST TRIGGER: type alias row type, template without substitutions
  const all = yield* sql<CommandIdRow>`SELECT command_id FROM commands`

  // MUST TRIGGER: statement stored without being yielded
  const statement = sql<CommandRow>`SELECT * FROM commands`

  // MUST TRIGGER: intersection row type
  const extended = yield* sql<CommandRow & { readonly extra: number }>`SELECT * FROM commands`

  // MUST TRIGGER: the tag is `sql.safe`, which is typed as the SqlClient itself
  const safe = yield* sql.safe<CommandRow>`SELECT * FROM commands`

  // MUST TRIGGER: the tag is the SqlClient returned by `withoutTransforms()`
  const raw = yield* sql.withoutTransforms()<CommandRow>`SELECT * FROM commands`

  // MUST TRIGGER: the client is aliased under another name; detection is by type, not identifier
  const db = sql
  const aliased = yield* db<CommandRow>`SELECT * FROM commands`

  // MUST TRIGGER: parenthesized tag
  const parenthesized = yield* (sql)<CommandRow>`SELECT * FROM commands`

  return [all, statement, extended, safe, raw, aliased, parenthesized] as const
})

// MUST TRIGGER: the client comes from a flatMap callback instead of a generator
export const viaFlatMap = SqlClient.SqlClient.pipe(
  Effect.flatMap((sql) => sql<CommandRow>`SELECT * FROM commands`)
)

// MUST TRIGGER: a plain parameter typed as the SqlClient interface
export const fromParameter = (sql: SqlClient.SqlClient) => sql<CommandRow>`SELECT * FROM commands`

// MUST TRIGGER: the tag is typed as Effect's Statement.Constructor, which SqlClient extends
declare const constructor: Statement.Constructor
export const fromConstructor = constructor<CommandRow>`SELECT * FROM commands`

// MUST TRIGGER: the row type is a type parameter of the enclosing function
export const generic = <A extends object>(sql: SqlClient.SqlClient) => sql<A>`SELECT * FROM commands`

// MUST TRIGGER: an explicit type argument inside SqlSchema's execute is still only asserted
export const insideSqlSchema = Effect.gen(function*() {
  const sql = yield* SqlClient.SqlClient
  return SqlSchema.findAll({
    Request: Schema.String,
    Result: Schema.Struct({ command_id: Schema.String }),
    execute: (key) => sql<CommandIdRow>`SELECT command_id FROM commands WHERE key_hash = ${key}`
  })
})

// MUST TRIGGER: typed sql tag inside an Effect.fn generator body
export const findCommand = Effect.fn("findCommand")(function*(key: string) {
  const sql = yield* SqlClient.SqlClient
  return yield* sql<CommandRow>`SELECT * FROM commands WHERE key_hash = ${key}`
})

export const dataFirstPipe = Effect.gen(function*() {
  const sql = yield* SqlClient.SqlClient
  // MUST TRIGGER: typed sql tag as the subject of a data-first pipe
  return yield* pipe(sql<CommandRow>`SELECT * FROM commands`, Effect.orDie)
})

// MUST NOT TRIGGER: untyped sql tag inside an Effect.fn generator body
export const findCommandUntyped = Effect.fn("findCommandUntyped")(function*(key: string) {
  const sql = yield* SqlClient.SqlClient
  return yield* sql`SELECT * FROM commands WHERE key_hash = ${key}`
})

export const negatives = Effect.gen(function*() {
  const sql = yield* SqlClient.SqlClient

  // MUST NOT TRIGGER: sql tag without a type argument
  const untyped = yield* sql`SELECT * FROM commands WHERE key_hash = ${keyHash}`

  // MUST NOT TRIGGER: sql tag without a type argument and without substitutions
  yield* sql`DELETE FROM commands`

  // MUST NOT TRIGGER: sql.unsafe with a type argument is a call, not a tagged template
  const unsafe = yield* sql.unsafe<CommandRow>("SELECT * FROM commands")

  // MUST NOT TRIGGER: identifier helper call, not a tagged template
  const table = sql("commands")

  // MUST NOT TRIGGER: fragment helpers nested in an untyped tagged template
  const filtered = yield* sql`SELECT * FROM ${table} WHERE ${sql.in("command_id", [minted])}`

  // MUST NOT TRIGGER: the recommended shape, SqlSchema with an untyped sql tag
  const findByKey = SqlSchema.findOne({
    Request: Schema.String,
    Result: Schema.Struct({ command_id: Schema.String, fingerprint: Schema.String }),
    execute: (key) => sql`SELECT command_id, fingerprint FROM commands WHERE key_hash = ${key}`
  })

  // MUST NOT TRIGGER: SqlSchema.void with an untyped sql tag
  const deleteByKey = SqlSchema.void({
    Request: Schema.String,
    execute: (key) => sql`DELETE FROM commands WHERE key_hash = ${key}`
  })

  // MUST NOT TRIGGER: the row type is given on the binding, not as a type argument of the tag
  const annotated: ReadonlyArray<object> = yield* sql`SELECT * FROM commands`

  return [untyped, unsafe, filtered, findByKey, deleteByKey, annotated] as const
})
