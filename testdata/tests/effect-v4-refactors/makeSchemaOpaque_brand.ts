// refactor: 4:14-4:23
import * as Schema from "effect/Schema"

export const ProductId = Schema.NonEmptyString.pipe(Schema.brand("ProductId"))
