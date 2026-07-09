// @effect-diagnostics redundantMapError:warning
import { Effect } from "effect"

type Todo = {
  readonly id: number
}

declare const encodeTodos: (todos: ReadonlyArray<Todo>) => Effect.Effect<string, unknown>
declare const makeDirectory: (path: string) => Effect.Effect<void, unknown>
declare const writeFile: (path: string, body: string) => Effect.Effect<void, unknown>

class TodosRepoError {
  constructor(readonly args: { cause: unknown } & Record<string, unknown>) {}
}

const filePath = "todos.json"

export const shouldReport = Effect.fn("TodosRepo.writeTodos")(function*(todos: ReadonlyArray<Todo>) {
  const encoded = yield* encodeTodos(todos).pipe(
    Effect.mapError((cause) => new TodosRepoError({ cause }))
  )

  yield* makeDirectory(filePath).pipe(
    Effect.mapError((cause) => new TodosRepoError({ cause }))
  )

  yield* writeFile(filePath, `${encoded}\n`).pipe(
    Effect.mapError((cause) => new TodosRepoError({ cause }))
  )
})

export const shouldNotReportDifferentMapper = Effect.gen(function*() {
  yield* makeDirectory(filePath).pipe(
    Effect.mapError((cause) => new TodosRepoError({ cause }))
  )

  yield* writeFile(filePath, "body").pipe(
    Effect.mapError((cause) => new TodosRepoError({ cause, filePath }))
  )
})

export const shouldNotReportLocalCapture = Effect.gen(function*() {
  const suffix = "!"

  yield* makeDirectory(filePath).pipe(
    Effect.mapError((cause) => new TodosRepoError({ cause, captured: suffix }))
  )

  yield* writeFile(filePath, suffix).pipe(
    Effect.mapError((cause) => new TodosRepoError({ cause, captured: suffix }))
  )
})

export const shouldNotReportMissingMapError = Effect.gen(function*() {
  yield* makeDirectory(filePath).pipe(
    Effect.mapError((cause) => new TodosRepoError({ cause }))
  )

  yield* writeFile(filePath, "body")
})

export const shouldReportMixedStyles = Effect.gen(function*() {
  yield* makeDirectory(filePath).pipe(
    Effect.mapError((cause) => new TodosRepoError({ cause }))
  )

  yield* Effect.mapError(writeFile(filePath, "body"), (cause) => new TodosRepoError({ cause }))
})
