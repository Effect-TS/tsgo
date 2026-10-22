import type * as SqlClient from "effect/sql/SqlClient"

type Text<Parts extends ReadonlyArray<string>, Index extends ReadonlyArray<unknown> = []> =
  Parts extends readonly [infer Head extends string, ...infer Tail extends ReadonlyArray<string>]
    ? Tail extends [] ? Head : `${Head}:p${Index["length"]}${Text<Tail, [...Index, unknown]>}`
    : string

type Values<Parts extends ReadonlyArray<string>, Params, Index extends ReadonlyArray<unknown> = []> =
  Parts extends readonly [string, ...infer Tail extends ReadonlyArray<string>]
    ? Tail extends [] ? []
      : [Params[`p${Index["length"]}` & keyof Params], ...Values<Tail, Params, [...Index, unknown]>]
    : []

// The compiler supplies literal segments; the existing public Query type owns SQL analysis.
export const tagged = <S extends SqlClient.QuerySchema>(query: SqlClient.Query<S>) => {
  type Params<Parts extends ReadonlyArray<string>> = NonNullable<Parameters<typeof query<Text<Parts>>>[1]>
  type Valid<Parts extends ReadonlyArray<string>> =
    Extract<Parts[number], `${string}:${string}`> extends never
      ? Parameters<typeof query<Text<Parts>>>[0] extends { readonly "SQL error": infer Message }
        ? { readonly "SQL error": Message }
        : unknown
      : { readonly "SQL error": "Use interpolations instead of named parameters" }

  type Tag = <const Parts extends ReadonlyArray<string>>(
    strings: TemplateStringsArray & Parts & Valid<NoInfer<Parts>>,
    ...values: Values<NoInfer<Parts>, Params<NoInfer<Parts>>>
  ) => ReturnType<typeof query<Text<Parts>>>

  return ((strings: TemplateStringsArray, ...values: ReadonlyArray<unknown>) => {
    let text = strings[0]
    const parameters: Record<string, unknown> = {}
    for (let i = 0; i < values.length; i++) {
      text += `:p${i}${strings[i + 1]}`
      parameters[`p${i}`] = values[i]
    }
    return (query as (text: string, parameters: Record<string, unknown>) => unknown)(text, parameters)
  }) as Tag
}
