// refactor: 11:9-11:10
import * as Effect from "effect/Effect"

type SqlResult = ReadonlyArray<{ id: number }>

const sql = (_strings: TemplateStringsArray, ..._args: ReadonlyArray<unknown>): Effect.Effect<SqlResult> =>
  Effect.succeed([])

export const insertEnvelope = {
  pg: () => (_row: { id: string }, messageId: string) =>
    sql`
      select * from messages where id = ${messageId}
    `.pipe(Effect.flatMap((rows) => {
      if (rows.length > 0) return Effect.succeed(rows)
      return sql`
        select * from messages where id = ${messageId}
      `
    }))
}
