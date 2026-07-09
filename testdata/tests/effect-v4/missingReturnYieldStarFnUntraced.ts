import { Effect } from "effect"

export const shouldComplainUntraced = Effect.fnUntraced(function*(){
    yield* Effect.fail("no") // <- should emit an error be return yield*
})

export const shouldComplainUntracedEager = Effect.fnUntracedEager(function*(){
    yield* Effect.fail("no") // <- should emit an error be return yield*
})

export const shouldNotComplainRegularFnUntraced = Effect.fnUntraced((a: string) => {
    return Effect.fail(a)
})
