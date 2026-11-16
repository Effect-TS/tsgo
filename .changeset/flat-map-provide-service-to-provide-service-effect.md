---
"@effect/tsgo": minor
---

Add the `flatMapProvideServiceToProvideServiceEffect` suggestion in `style`. An `Effect.flatMap` (or `Effect.andThen` given a callback) whose callback only forwards its parameter to `Effect.provideService` is a manual `Effect.provideServiceEffect`. The rule reports it in every spelling: pipeable, data-first, `pipe(...)` and curried, on both the outer `flatMap` and the inner `provideService`, and inside `Effect.gen` / `Effect.fn` bodies.

```ts
makeConnection.pipe(Effect.flatMap((conn) => Effect.provideService(program, Database, conn))) // reported
Effect.provideServiceEffect(program, Database, makeConnection)                                // ok
```

A quick fix rewrites the expression to `Effect.provideServiceEffect(self, Key, acquire)` and keeps any later pipe steps: `Effect.suspend(() => step(s)).pipe(Effect.flatMap((meta) => Effect.provideService(effect, Key, meta)), Effect.map(f))` becomes `Effect.provideServiceEffect(effect, Key, Effect.suspend(() => step(s))).pipe(Effect.map(f))`.

The callback is not reported when it does anything else: when the parameter appears outside the service argument (in the program, in any earlier step of the program, or in the key), when the program or key uses something the callback binds (the name of a named function expression, or a function-expression callback's own `this`, `super`, `new.target` or `arguments`, plain or as shorthand `{ arguments }`), when the service argument is not the bare parameter, when steps follow `provideService`, when the acquired value's type is not assignable to the key's service type (for example through a `(conn: any)` parameter), or when the `flatMap` result type differs from the `provideServiceEffect` one (a return-type annotation on the callback or explicit type arguments that widen it, or explicit type arguments on `provideService`), since code after it may rely on that type. After the fix, the program expression is evaluated once when the expression is built rather than on every run of the callback. The `Effect.gen` form (`const p = yield* acquire; return yield* Effect.provideService(self, Key, p)`) is deliberately not reported. On Effect v3, data-first `Effect.andThen(acquire, callback)` is not reported, since its overloads do not form a dual. The rule supports Effect v3 and v4.

Piping-flow fixes can now build an expression for a flow prefix with `PipingFlowPrefixExpression`, including prefixes that end inside a `.pipe(...)` or `pipe(...)` call.
