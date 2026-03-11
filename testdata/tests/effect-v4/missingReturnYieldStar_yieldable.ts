import { Effect, Option } from "effect"

export const shouldComplain = (n: number) =>
    Effect.gen(function*() {
        if (n === 0) {
            yield* Option.none() // should trigger because Option<A> is yieldable
        }
        return n / 1
    })
