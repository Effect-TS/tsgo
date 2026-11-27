// @effect-diagnostics *:off
// @effect-diagnostics sqlTaggedRowTypeToSqlSchema:warning
import { Effect } from "effect"
import { SqlClient } from "effect/sql"

interface CommandRow {
  readonly command_id: string
}

export const program = Effect.gen(function*() {
  const sql = yield* SqlClient.SqlClient
  return yield* sql<CommandRow>`SELECT command_id FROM commands`
})
