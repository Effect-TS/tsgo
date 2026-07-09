// @effect-v3
import * as Schema from "effect/Schema"

export class Valid extends Schema.Class<Valid>("Valid")({
  a: Schema.Finite
}) {
  protected constructor(a: Valid, options: Schema.MakeOptions) {
    super(a, options)
  }
}
