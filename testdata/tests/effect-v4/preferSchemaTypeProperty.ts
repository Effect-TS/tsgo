// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service"
      }
    ]
  }
}
// @filename: test.ts
// @effect-diagnostics preferSchemaTypeProperty:error
import { Schema, Schema as S } from "effect"

export const AddressId = Schema.String
export type AddressId = Schema.Schema.Type<typeof AddressId>

export const Address = Schema.Struct({ id: AddressId })
export type Address = S.Schema.Type<typeof Address>

namespace Models {
  export const BusinessId = Schema.String
}
type BusinessId = Schema.Schema.Type<typeof Models.BusinessId>

type GoodTypeQuery = typeof Address.Type
type GoodSchemaType = Schema.Type<typeof Address>

declare namespace LocalSchema {
  namespace Schema {
    type Type<T> = T
  }
}
type Unrelated = LocalSchema.Schema.Type<typeof Address>
