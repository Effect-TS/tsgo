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
// @filename: schemas.ts
import { Schema } from "effect"

export const UserId = Schema.Struct({})
export type UserId = typeof UserId.Type

export const SessionId = Schema.Struct({})

export namespace UsersRepo {
  export const User = Schema.Struct({ id: Schema.Finite })
  export type User = typeof User.Type
}

export namespace OrdersRepo {
  export const Order = Schema.Struct({ id: Schema.Finite })
  export type Order = { readonly id: string }
}
// @filename: test.ts
// @effect-diagnostics unnecessaryTypeofType:warning
import { OrdersRepo, SessionId, UserId, UsersRepo } from "./schemas"
import * as Schemas from "./schemas"
import { Schema } from "effect"

const a: typeof UserId.Type = {}
const b: typeof UsersRepo.User.Type = { id: 1 }
const namespaceImportTrigger: typeof Schemas.UsersRepo.User.Type = { id: 1 }
const shouldNotTriggerMissingType: typeof SessionId.Type = {}
const shouldNotTriggerMismatchedType: typeof OrdersRepo.Order.Type = { id: 1 }

const c: UserId = {}
const d: UsersRepo.User = { id: 1 }

// https://github.com/Effect-TS/tsgo/issues/991
// an interface cannot extend itself, so this must not be reported
export const IssueStruct = Schema.Struct({ flag: Schema.Literal(true) })
export type Identity<A> = A
export interface IssueStruct extends Identity<typeof IssueStruct.Type> {}
// extending a different named type is still reported
export interface ExtendsOther extends Identity<typeof UserId.Type> {}
