// @effect-v3
// @effect-diagnostics typeUnionToSchemaUnion:warning
import { Schema } from "effect"

export const Circle = Schema.Struct({ kind: Schema.Literal("circle"), radius: Schema.Number })
export const Square = Schema.Struct({ kind: Schema.Literal("square"), side: Schema.Number })
export const Triangle = Schema.Struct({ kind: Schema.Literal("triangle"), base: Schema.Number })

// MUST TRIGGER - exported union of `typeof S.Type`; the fix keeps `export` and
// inserts `export const Shape = Schema.Union(Circle, Square)` just before the alias
export type Shape = typeof Circle.Type | typeof Square.Type

// MUST TRIGGER - union of `typeof S.Encoded`; the fixed alias uses `.Encoded`
export type ShapeEncoded = typeof Circle.Encoded | typeof Square.Encoded

// MUST TRIGGER - non-exported alias with three members; the fix inserts a non-exported const
type AnyShape = typeof Circle.Type | typeof Square.Type | typeof Triangle.Type

// MUST TRIGGER - nested parenthesized unions are flattened into a single Schema.Union
type FlatShape = (typeof Circle.Type | typeof Square.Type) | (typeof Triangle.Type)

// MUST TRIGGER - leading `|` and parenthesized members
type LeadingPipe =
  | (typeof Circle.Encoded)
  | typeof Square.Encoded

// MUST TRIGGER - members are property-access expressions on the Schema module
type Primitive = typeof Schema.String.Type | typeof Schema.Number.Type

// MUST TRIGGER - members are Schema classes
class Person extends Schema.Class<Person>("Person")({ name: Schema.String }) {}
class Robot extends Schema.Class<Robot>("Robot")({ serial: Schema.Number }) {}
export type Being = typeof Person.Type | typeof Robot.Type

// MUST TRIGGER - members are qualified through a local namespace
namespace Models {
  export const Cat = Schema.Struct({ meow: Schema.Boolean })
  export const Dog = Schema.Struct({ bark: Schema.Boolean })
}
type Pet = typeof Models.Cat.Type | typeof Models.Dog.Type

// MUST TRIGGER, no fix offered - the alias is declared before its member schemas;
// a const inserted before the alias would fail with TS2448 (used before declaration)
export type Late = typeof LateA.Type | typeof LateB.Type
export const LateA = Schema.Struct({ a: Schema.String })
export const LateB = Schema.Struct({ b: Schema.String })

// MUST TRIGGER, no fix offered - one member is declared earlier and one (`LateD`) later
type HalfLate = typeof Circle.Type | typeof LateD.Type
const LateD = Schema.Struct({ d: Schema.String })

// MUST TRIGGER, no fix offered - the members are qualified through a namespace declared after the alias
type LateQualified = typeof LateModels.A.Type | typeof LateModels.B.Type
namespace LateModels {
  export const A = Schema.String
  export const B = Schema.Boolean
}

// MUST TRIGGER - a value named `Pair` already exists, so the fix names the schema `PairSchema`
export const Pair = [Circle, Square] as const
export type Pair = typeof Circle.Type | typeof Square.Type

// MUST TRIGGER - both `Duo` and `DuoSchema` already exist, so the fix names the schema `DuoSchema2`
const Duo = "duo"
const DuoSchema = "duoSchema"
type Duo = typeof Circle.Encoded | typeof Square.Encoded

// MUST TRIGGER - non-ambient namespace: the fix inserts `export const Any` inside the namespace
namespace Shapes {
  export type Any = typeof Circle.Type | typeof Square.Type
}

// MUST TRIGGER - type alias local to a function body
export const localAlias = () => {
  type Local = typeof Circle.Type | typeof Square.Type
  const value: Local = { kind: "circle", radius: 1 }
  return value
}

// MUST TRIGGER - union of `Schema.Schema.Type<typeof S>` members; the fix uses
// `Schema.Union(Circle, Square)` and the alias becomes `typeof HelperShape.Type`
type HelperShape = Schema.Schema.Type<typeof Circle> | Schema.Schema.Type<typeof Square>

// MUST TRIGGER - `typeof S.Type` and `Schema.Schema.Type<typeof S>` mixed: both extract
// the Type side, so the fix uses `Schema.Union(Circle, Square, Triangle)`
type MixedForms = typeof Circle.Type | Schema.Schema.Type<typeof Square> | typeof Triangle.Type

// MUST TRIGGER - union of `Schema.Schema.Encoded<typeof S>` members; the alias becomes
// `typeof HelperShapeEncoded.Encoded`
type HelperShapeEncoded = Schema.Schema.Encoded<typeof Circle> | Schema.Schema.Encoded<typeof Square>

// MUST TRIGGER - `typeof S.Encoded` and `Schema.Schema.Encoded<typeof S>` mixed: both
// extract the Encoded side
type MixedFormsEncoded = Schema.Schema.Encoded<typeof Circle> | typeof Square.Encoded

// MUST TRIGGER - `Schema.Schema.Type` with a qualified, non-local member expression
type HelperPrimitive = Schema.Schema.Type<typeof Schema.String> | Schema.Schema.Type<typeof Schema.Number>

// MUST TRIGGER - the alias does not start its line (inline function body); the fix
// inserts the const inline, right before the alias, without duplicating the line prefix
export const inlineBody = () => { type InlineBody = typeof Circle.Type | typeof Square.Type; return [] as Array<InlineBody> }

// MUST TRIGGER - inline namespace body; the const is inserted inline before the alias
namespace InlineShapes { export type Any = typeof Circle.Type | typeof Square.Type }

// MUST TRIGGER - another statement precedes the alias on the same line
const sameLineBefore = 1; type SameLine = typeof Circle.Encoded | typeof Square.Encoded

// MUST TRIGGER - a block comment precedes the alias on the same line
/* shapes */ type Commented = typeof Circle.Type | typeof Square.Type

// MUST TRIGGER - the fix inserts the const before the JSDoc, which stays attached to the alias
/** Either a circle or a square */
type Documented = typeof Circle.Type | typeof Square.Type

// MUST NOT TRIGGER - the recommended form: the type is derived from a Schema.Union
export const Shape2 = Schema.Union(Circle, Square)
export type Shape2 = typeof Shape2.Type

// MUST NOT TRIGGER - a single `typeof S.Type` member is not a union
type OnlyCircle = typeof Circle.Type

// MUST NOT TRIGGER - parentheses around a single member do not make a union
type OnlyCircleParens = (typeof Circle.Type)

// MUST NOT TRIGGER - mixed `.Type` and `.Encoded`
type Mixed = typeof Circle.Type | typeof Square.Encoded

// MUST NOT TRIGGER - mixed member forms that extract different sides
type MixedFormsMixedSides = typeof Circle.Type | Schema.Schema.Encoded<typeof Square>

// MUST NOT TRIGGER - `Schema.Schema.Context<typeof S>` is not an accepted member form
type HelperContext = Schema.Schema.Context<typeof Circle> | Schema.Schema.Context<typeof Square>

// MUST NOT TRIGGER - `Schema.Schema.Type` from an unrelated local namespace is not the
// Effect helper (here it returns the schema itself, not its Type)
declare namespace LocalSchema {
  namespace Schema {
    type Type<T> = T
  }
}
type UnrelatedHelper = LocalSchema.Schema.Type<typeof Circle> | LocalSchema.Schema.Type<typeof Square>

// MUST NOT TRIGGER - `Schema.Schema.Type` applied to a non-schema value
type HelperFake = Schema.Schema.Type<typeof NotSchemaA> | Schema.Schema.Type<typeof NotSchemaB>

// MUST NOT TRIGGER - `| null` is not a schema member
type Nullable = typeof Circle.Type | typeof Square.Type | null

// MUST NOT TRIGGER - literal member
type WithLiteral = typeof Circle.Type | typeof Square.Type | "none"

// MUST NOT TRIGGER - interface member
interface Extra {
  readonly kind: "extra"
}
type WithInterface = typeof Circle.Type | Extra

// MUST NOT TRIGGER - union of type references that happen to resolve to schema types
type CircleType = typeof Circle.Type
type SquareType = typeof Square.Type
type ViaAliases = CircleType | SquareType

// MUST NOT TRIGGER - union of the schemas themselves, not their extracted types
type SchemaValues = typeof Circle | typeof Square

// MUST NOT TRIGGER - indexed access is another member form
type Indexed = (typeof Circle)["Type"] | (typeof Square)["Type"]

// MUST NOT TRIGGER - other schema properties
type Services = typeof Circle.Context | typeof Square.Context

// MUST NOT TRIGGER - intersection, not a union
type Both = typeof Circle.Type & typeof Square.Type

// MUST NOT TRIGGER - the union is nested inside another type, not the alias right-hand side
type ShapeList = Array<typeof Circle.Type | typeof Square.Type>
type ShapeHolder = { readonly shape: typeof Circle.Type | typeof Square.Type }

// MUST NOT TRIGGER - generic alias
type Generic<_T> = typeof Circle.Type | typeof Square.Type

// MUST NOT TRIGGER - ambient context
declare namespace Ambient {
  type AmbientShape = typeof Circle.Type | typeof Square.Type
}

// MUST NOT TRIGGER - `Type` / `Encoded` statics on non-schema values (detection is
// checker-based, not name-based)
class NotSchemaA {
  static Type = "a" as const
  static Encoded = 1
}
class NotSchemaB {
  static Type = "b" as const
  static Encoded = 2
}
type Fake = typeof NotSchemaA.Type | typeof NotSchemaB.Type
const FakeObject = { Type: "x" as const, Encoded: 0 }
type FakeObjects = typeof FakeObject.Type | typeof NotSchemaA.Type

// MUST NOT TRIGGER - one schema member and one non-schema member
type HalfFake = typeof Circle.Type | typeof NotSchemaA.Type
