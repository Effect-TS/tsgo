import { Effect } from "effect"

export class Test {
    a: number = 42
    shouldComplain = Effect.gen({self: this}, function*(){
        yield* Effect.fail(this.a) // <- should emit an error be return yield*
    })
}