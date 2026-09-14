// @effect-diagnostics *:off
// @effect-diagnostics obsoleteMatchImport:warning
import { Effect } from "effect"
// @ts-expect-error - @effect/match is obsolete in v4
import * as Match from "@effect/match"

export const preview = Effect.sync(() => Match)
