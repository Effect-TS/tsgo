import { Effect } from "effect"

export const program = Effect.gen(function*(){
    // @effect-diagnostics-next-line floatingEffect:off
   yield* Effect.log("hello")
})