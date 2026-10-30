// @filename: tsconfig.json
{
  "compilerOptions": {
    "target": "ES2025",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "noEmit": true,
    "skipLibCheck": true,
    "strict": true,
    "plugins": [
      {
        "name": "@effect/language-service",
        "diagnosticSeverity": {
          "effectInVoidSuccess": "warning"
        }
      }
    ]
  }
}

// @filename: issue381_effectInVoidSuccess_ts2589.ts
import type { Schema } from "effect"

type Recurse<Value> = {
  [Key in keyof Value]: Recurse<Value[Key]>
}

type Json = readonly Json[] | JsonObject
type JsonObject = { readonly [key: string]: Json }

type ValueType<Definition> =
  Definition extends Schema.Codec<infer Value, Json> ? Value : never

declare let value: Recurse<
  ValueType<Schema.Codec<JsonObject>> | null
>

value = null
