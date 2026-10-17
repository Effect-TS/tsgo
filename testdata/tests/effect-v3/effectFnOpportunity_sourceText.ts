// @filename: tsconfig.json
{"compilerOptions":{"plugins":[{"name":"@effect/language-service","effectFn":["span","untraced","no-span","inferred-span","suggested-span"]}]}}

// @filename: effectFnOpportunity_sourceText.ts
import { Effect } from "effect"

export const signature = <T /* generic */>(
  value: T,
  // default parameter
  n = 1_000 /* units */,
) => {
  // before return
  return Effect.gen(function*() {
    // keep the body verbatim
    yield* Effect.log(value)
    return n + 0xff
  }).pipe(
    // before map
    Effect.map((n) => n + 2_000) /* after map */,
    Effect.withSpan('custom-span'),
  )
  // after return
}

export function declaration(n: number) {
  return Effect.gen(function*() {
    // declaration body
    return (yield* Effect.succeed(n)) + 1_000
  })
}

export const regular = (n: number) => {
  // ordinary function body
  const a = 1_000
  const b = 0xff
  const c = 'single quotes'
  const d = /a\/\/b/
  const e = `template ${c}`
  return Effect.succeed({ n, a, b, c, d, e })
}

export const bare: (n: number) => Effect.Effect<number> = n => Effect.gen(function*() {
  // unparenthesized parameter
  const result = yield* Effect.succeed(n)
  return result
})

export const traced = (n: number) => Effect.gen(function*() {
  yield* Effect.log("traced", n)
  return 1_000
}).pipe(
  Effect.withSpan(
    // span expression
    `span-${"name" /* inside span */}-${/[/][*]/.source}`,
    { attributes: { pattern: /[/][/]/.source } },
  ),
)
