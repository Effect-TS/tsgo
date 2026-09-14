// @filename: tsconfig.json
{
  "compilerOptions": {
    "types": ["node"],
    "plugins": [
      {
        "name": "@effect/language-service",
        "ignoreEffectErrorsInTscExitCode": true,
        "skipDisabledOptimization": true
      }
    ]
  }
}

// @filename: obsoleteMatchImport.ts
import { Effect } from "effect"

// Flagged: obsolete @effect/match imports in Effect v4
// @ts-expect-error - @effect/match not installed in v4 fixture
import * as Match1 from "@effect/match"
// @ts-expect-error - @effect/match not installed in v4 fixture
import { type Match as Match2 } from "@effect/match"
// @ts-expect-error - @effect/match not installed in v4 fixture
import * as MatchSub from "@effect/match/Matcher"
// @ts-expect-error - @effect/match not installed in v4 fixture
import "@effect/match"

// Flagged: top-level require
export const matchModule = require("@effect/match")

// Flagged: export from @effect/match
// @ts-expect-error - @effect/match not installed in v4 fixture
export * from "@effect/match"

// Not flagged: modern v4 Match imports from effect
import { Match } from "effect"
import * as MatchFromEffect from "effect/Match"

// Not flagged: unrelated packages
// @ts-expect-error - third-party package
import * as Other from "@other/match"

export const program = Effect.sync(() => ({
  val: Match.value(1),
  m1: Match1,
  mSub: MatchSub,
  fromEffect: MatchFromEffect,
  other: Other
}))
export type M2 = Match2
