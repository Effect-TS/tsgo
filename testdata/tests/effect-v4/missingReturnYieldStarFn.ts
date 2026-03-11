import { Effect } from "effect"

export const shouldComplain = Effect.fn(function*(){
    yield* Effect.fail("no") // <- should emit an error be return yield*
})

export const shouldComplainNamedFn = Effect.fn("named")(function* namedFn(){
    yield* Effect.fail("no") // <- should emit an error be return yield*
})

export const shouldNotComplainRegularFn = Effect.fn((a: string) => {
    return Effect.fail(a)
})

export class Test {
    a: number = 42
    shouldComplain = Effect.fn({self: this}, function*(){
        yield* Effect.fail(this.a) // <- should emit an error be return yield*
    })
    shouldComplainNamedFn = Effect.fn("named")({self: this}, function* namedFn(){
        yield* Effect.fail(this.a) // <- should emit an error be return yield*
    })
}
export const shouldComplainNamedFnWithPipes = Effect.fn("named")(function* namedFn(){
    yield* Effect.fail("no") // <- should emit an error be return yield*
}, Effect.ignore)