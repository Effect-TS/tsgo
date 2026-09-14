// @effect-diagnostics *:off
// @effect-diagnostics obsoleteSchemaImport:warning
import { Effect } from "effect"
// @ts-expect-error - @effect/schema is obsolete in v4
import * as S from "@effect/schema/Schema"

export const preview = Effect.sync(() => S)
