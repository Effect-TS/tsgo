import { Effect, Option } from "effect"

export const program = Effect.gen(function*(){
    const option = yield* Option.some(1)
    return option
})

export const gen = Option.gen(function*(){
    const option = yield* Option.some(1)
    return option
})
