import { Schema } from "effect"
import { Opaque as SchemaOpaque } from "effect/Schema"
import * as S from "effect/Schema"

class Valid extends S.Opaque<Valid>()(S.Struct({ value: S.String })) {
  static value = "static"
  static method() {}
}

class Invalid extends Schema.Opaque<Invalid>()(Schema.Struct({ value: Schema.String })) {
  value = "instance"
  method() {}
  get computed() {
    return this.value
  }
  set computed(value: string) {
    this.value = value
  }
}

class Aliased extends SchemaOpaque<Aliased>()(Schema.Struct({})) {
  field = true
}

const Expression = class extends S.Opaque<Invalid>()(S.Struct({})) {
  expressionField = true
}

declare function Opaque<Self>(): (schema: unknown) => new() => object
class Unrelated extends Opaque<Unrelated>()({}) {
  allowed = true
}

class WrongShape extends S.Class<WrongShape>("WrongShape")({}) {
  allowed = true
}

void Expression
