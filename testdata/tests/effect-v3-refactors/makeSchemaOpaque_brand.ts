// refactor: 5:14-5:23
// @effect-v3
import * as Schema from "effect/Schema"

export const ProductId = Schema.NonEmptyString.pipe(Schema.brand("ProductId"))
