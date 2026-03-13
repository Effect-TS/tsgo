import { Data} from "effect"

type TE<T> = Data.TaggedEnum<{
    A: { a: T }
    B: { b?: T }
  }>

  export interface TEDefinition extends Data.TaggedEnum.WithGenerics<1> {
    readonly taggedEnum: TE<this["A"]>
  }