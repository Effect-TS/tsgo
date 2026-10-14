// @filename: package.json
{ "name": "@effect/test-app", "version": "1.0.0" }

// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "barrelImportPackages": ["effect"]
      }
    ]
  }
}

// @filename: test.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferBarrelImport:warning

// reported: namespace import of a module
import * as Record from "effect/Record"
import * as O from "effect/Option"
import type * as Sch from "effect/Schema"
import * as HttpClient from "effect/http/HttpClient"

// reported once per declaration: named imports from a module
import { map, filter } from "effect/Array"
import type { Duration } from "effect/Duration"
import { make as makeClock } from "effect/testing/TestClock"

// not reported: imports from a barrel
import { Effect, pipe } from "effect"
import { TestClock } from "effect/testing"
import * as Http from "effect/http"

// not reported: side-effect imports
import "effect/Order"

// not reported: packages outside barrelImportPackages
import * as VitestUtils from "@effect/vitest/utils"

export const usages = [Record, O, HttpClient, map, filter, makeClock, Effect, pipe, TestClock, Http, VitestUtils]
export type Usages = [Sch.Top, Duration]
