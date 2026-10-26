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
// @effect-diagnostics deterministicKeys:error
import { Data, Schema } from "effect"

export class Started extends Schema.TaggedClass<Started, { readonly brand: unique symbol }>()("Started", {}) {}

export class Completed extends Schema.TaggedClass<Completed, { readonly brand: unique symbol }>()("completed", {}) {}

export class Cancelled extends Schema.TaggedClass<Cancelled, { readonly brand: unique symbol }>("CancelledIdentifier")("cancelled", {}) {}

export class Queued extends Data.TaggedClass("Queued")<{}> {}

export class Failed extends Data.TaggedClass("failed")<{}> {}

export class DataError extends Data.TaggedError("data-error")<{}> {}

export class SchemaError extends Schema.TaggedError<SchemaError, { readonly brand: unique symbol }>()("schema-error", {}) {}
