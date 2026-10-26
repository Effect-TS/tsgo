// @filename: package.json
{ "name": "@effect/test-app", "version": "1.0.0" }

// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "keyPatterns": [
          { "target": "taggedClass", "pattern": "identifier" }
        ]
      }
    ]
  }
}

// @filename: test.ts
// @effect-v3
// @effect-diagnostics deterministicKeys:error
import * as Data from "effect/Data"
import * as Schema from "effect/Schema"

export class Started extends Schema.TaggedClass<Started>()("Started", {}) {}

export class Completed extends Schema.TaggedClass<Completed>()("completed", {}) {}

export class Cancelled extends Schema.TaggedClass<Cancelled>("CancelledIdentifier")("cancelled", {}) {}

export class Queued extends Data.TaggedClass("Queued")<{}> {}

export class Failed extends Data.TaggedClass("failed")<{}> {}

export class Fetch extends Schema.TaggedRequest<Fetch>()("fetch", {
  payload: {},
  success: Schema.Void,
  failure: Schema.Never
}) {}

export class DataError extends Data.TaggedError("data-error")<{}> {}

export class SchemaError extends Schema.TaggedError<SchemaError>()("schema-error", {}) {}
