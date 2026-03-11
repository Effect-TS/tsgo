import { Effect } from "effect"

export const program = Effect.gen(function*(){
   Effect.log("hello") // <- this should be yielded instead
})