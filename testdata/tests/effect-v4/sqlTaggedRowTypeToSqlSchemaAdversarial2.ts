// @effect-diagnostics *:off
// @effect-diagnostics sqlTaggedRowTypeToSqlSchema:warning
import { SqlClient } from "effect/sql"

interface Row {
  readonly id: string
}

interface PrimaryClient extends SqlClient.SqlClient {
  readonly connection: "primary"
}
interface ReplicaClient extends SqlClient.SqlClient {
  readonly connection: "replica"
}
declare const sql: PrimaryClient | ReplicaClient

// Both union members are Effect SqlClients; choosing a connection does not validate rows.
// MUST TRIGGER: an explicit row type on a union of Effect SqlClient extensions.
export const typed = sql<Row>`SELECT id FROM rows`

// MUST NOT TRIGGER: the same union client without an explicit row type.
export const untyped = sql`SELECT id FROM rows`
