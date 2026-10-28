// @filename: schemas.ts
// @effect-v3
import { Schema } from "effect"

export const Circle = Schema.Struct({ kind: Schema.Literal("circle"), radius: Schema.Number })
export const Square = Schema.Struct({ kind: Schema.Literal("square"), side: Schema.Number })
export const Shape = Schema.Union(Circle, Square)

// @filename: test.ts
// @effect-v3
// @effect-diagnostics typeUnionToSchemaUnion:warning
import { Circle, Shape, Square } from "./schemas"

// MUST TRIGGER - members are imported and no Schema module is imported here: the fix
// uses the `FindModuleReference` fallback `Schema` reference and adds no import
export type Figure = typeof Circle.Type | typeof Square.Type

// MUST TRIGGER, no fix offered - `Hexagon` is declared after the alias in this file
type Polygon = typeof Square.Type | typeof Hexagon.Type
const Hexagon = Square

// MUST NOT TRIGGER - already derived from the imported Schema.Union
type DerivedShape = typeof Shape.Type
