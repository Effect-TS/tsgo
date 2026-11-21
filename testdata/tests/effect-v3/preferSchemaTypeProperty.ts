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
// @effect-v3
// @effect-diagnostics preferSchemaTypeProperty:error
import { Schema, Schema as S } from "effect"

export const AddressId = Schema.String
export type AddressId = Schema.Schema.Type<typeof AddressId>

export const Address = Schema.Struct({ id: AddressId })
export type Address = S.Schema.Type<typeof Address>

type GoodTypeQuery = typeof Address.Type
type GoodSchemaType = Schema.Type<typeof Address>

export type AddressEncoded = Schema.Schema.Encoded<typeof Address>
export type AddressContext = S.Schema.Context<typeof Address>

// https://github.com/Effect-TS/tsgo/issues/1074
// `typeof X.Type` is not valid as a heritage clause entry, so these must not be reported
export const Person = Schema.Struct({ name: Schema.String })
export interface Person extends Schema.Schema.Type<typeof Person> {}
export interface PersonEncoded extends S.Schema.Encoded<typeof Person> {}
export class PersonImpl implements Schema.Schema.Type<typeof Person> {
  name = ""
}
// a type argument inside a heritage clause entry is still reported
export interface ReadonlyPerson extends Readonly<Schema.Schema.Type<typeof Person>> {}
