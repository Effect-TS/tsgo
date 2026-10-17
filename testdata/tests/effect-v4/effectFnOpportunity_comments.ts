// @filename: tsconfig.json
{"compilerOptions":{"plugins":[{"name":"@effect/language-service","effectFn":["span","untraced","no-span","inferred-span","suggested-span"]}]}}

// @filename: effectFnOpportunity_comments.ts
import { Effect } from "effect"

export const f = (n: number) =>
  Effect.gen(function* () {
    // explains the next line
    const x = yield* Effect.succeed(n) // trailing note
    /* block note */
    return x + 1_000
  })
