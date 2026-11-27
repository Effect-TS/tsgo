// @effect-diagnostics *:off
// @effect-diagnostics sqlTaggedRowTypeToSqlSchema:warning
import { SqlClient, Statement } from "effect/sql"

interface Row {
  readonly id: string
}

// A client extension can redeclare the template overload while retaining all
// Constructor members and its identifier overload.
interface ExtendedConstructor extends Statement.Constructor {
  <A extends object = Record<string, unknown>>(strings: TemplateStringsArray, ...args: Array<any>): Statement.Statement<A>
  (value: string): Statement.Identifier
}
declare const sql: SqlClient.SqlClient
const extended: ExtendedConstructor = sql

// MUST TRIGGER: the tag's type explicitly extends Effect's Statement.Constructor.
export const fromExtension = extended<Row>`SELECT id FROM rows`

// MUST NOT TRIGGER: the same extended constructor without a row type argument.
export const untypedExtension = extended`SELECT id FROM rows`

