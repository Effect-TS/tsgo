// refactor: 5:18-5:26
// @effect-v3
import * as Schema from "effect/Schema"

export interface MyStruct {
  anyProp: any
  bigintProp: bigint
  booleanProp: boolean
  neverProp: never
  nullProp: null
  numberProp: number
  stringProp: string
  undefinedProp: undefined
  unknownProp: unknown
  voidProp: void
  arrayTypeProp: Array<string>
  arrayProp: Array<string>
  readonlyArrayProp: ReadonlyArray<string>
  arrayWithReadonlyProp: readonly string[]
  dateProp: Date
  trueProp: true
  falseProp: false
  literalProp: "hello"
  numericLiteralProp: 42
  unionProp: string | boolean
  intersectionProp: { a: string } & { b: number }
  recordProp: Record<string, number>
  inlineStruct: { a: string; b: number }
  optionalProp?: string
  prop: string
  prop2: string
}
