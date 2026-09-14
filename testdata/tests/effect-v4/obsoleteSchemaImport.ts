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

// @filename: obsoleteSchemaImport.ts
import { Effect } from "effect"

// Flagged: obsolete @effect/schema imports in Effect v4
// @ts-expect-error - @effect/schema not installed in v4 fixture
import * as S1 from "@effect/schema"
// @ts-expect-error - @effect/schema not installed in v4 fixture
import * as S2 from "@effect/schema/Schema"
// @ts-expect-error - @effect/schema not installed in v4 fixture
import * as AST from "@effect/schema/AST"
// @ts-expect-error - @effect/schema not installed in v4 fixture
import { type Schema as SType } from "@effect/schema"
// @ts-expect-error - @effect/schema not installed in v4 fixture
import "@effect/schema"

// Flagged: top-level require
export const schemaModule = require("@effect/schema/Schema")

// Flagged: export from @effect/schema
// @ts-expect-error - @effect/schema not installed in v4 fixture
export * from "@effect/schema"

// Not flagged: modern v4 Schema imports from effect
import { Schema } from "effect"
import * as SchemaFromEffect from "effect/Schema"
import * as SchemaAST from "effect/SchemaAST"

// Not flagged: unrelated packages
// @ts-expect-error - third-party package
import * as Other from "@other/schema"

export const program = Effect.sync(() => ({
  val: Schema.Struct({ a: Schema.String }),
  s1: S1,
  s2: S2,
  ast: AST,
  fromEffect: SchemaFromEffect,
  schemaAst: SchemaAST,
  other: Other
}))
export type S = SType
