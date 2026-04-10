// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "effectFn": ["inferred-span"]
      }
    ]
  }
}

// @filename: effect_fn_opportunity_panic.ts
import { Data, Effect, flow, Layer, Schema, Context } from "effect"
import { FetchHttpClient, HttpClient, HttpClientRequest, HttpClientResponse } from "effect/unstable/http"

class Todo extends Schema.Class<Todo>("Todo")({
  userId: Schema.Number,
  id: Schema.Number,
  title: Schema.String,
  completed: Schema.Boolean
}) {}

class User extends Schema.Class<User>("User")({
  id: Schema.Number,
  name: Schema.String,
  email: Schema.String
}) {}

class RequestError extends Data.TaggedError("RequestError")<{
  readonly url: string
  readonly cause: unknown
}> {}

class InvalidStatusError extends Data.TaggedError("InvalidStatusError")<{
  readonly url: string
  readonly status: number
}> {}

class DecodeError extends Data.TaggedError("DecodeError")<{
  readonly url: string
  readonly cause: unknown
}> {}

type JsonPlaceholderError = RequestError | InvalidStatusError | DecodeError

class ApiConfig extends Context.Service<ApiConfig, {
  readonly baseUrl: string
}>()("ApiConfig") {}

const ApiConfigLive = Layer.succeed(ApiConfig)({
  baseUrl: "https://jsonplaceholder.typicode.com"
})

class JsonPlaceholder extends Context.Service<JsonPlaceholder, {
  readonly getTodo: (id: number) => Effect.Effect<Todo, JsonPlaceholderError>
  readonly getUser: (id: number) => Effect.Effect<User, JsonPlaceholderError>
}>()("JsonPlaceholder") {}

const JsonPlaceholderLive = Layer.effect(JsonPlaceholder)(
  Effect.gen(function* () {
    const config = yield* ApiConfig
    const client = (yield* HttpClient.HttpClient).pipe(
      HttpClient.mapRequest(
        flow(
          HttpClientRequest.prependUrl(config.baseUrl),
          HttpClientRequest.acceptJson
        )
      ),
      HttpClient.filterStatusOk
    )

    return {
      getTodo: (id: number) => {
        const path = `/todos/${id}`
        const url = new URL(path, config.baseUrl).toString()

        return client.get(path).pipe(
          Effect.flatMap(HttpClientResponse.schemaBodyJson(Todo)),
          Effect.mapError((cause) => {
            if (cause._tag === "HttpClientError") {
              if (cause.reason._tag === "StatusCodeError") {
                return new InvalidStatusError({
                  url,
                  status: cause.reason.response.status
                })
              }

              return new RequestError({ url, cause })
            }

            return new DecodeError({ url, cause })
          })
        )
      },
      getUser: (id: number) => {
        const path = `/users/${id}`
        const url = new URL(path, config.baseUrl).toString()

        return client.get(path).pipe(
          Effect.flatMap(HttpClientResponse.schemaBodyJson(User)),
          Effect.mapError((cause) => {
            if (cause._tag === "HttpClientError") {
              if (cause.reason._tag === "StatusCodeError") {
                return new InvalidStatusError({
                  url,
                  status: cause.reason.response.status
                })
              }

              return new RequestError({ url, cause })
            }

            return new DecodeError({ url, cause })
          })
        )
      }
    }
  })
).pipe(Layer.provide(Layer.mergeAll(ApiConfigLive, FetchHttpClient.layer)))

const program = Effect.gen(function* () {
  const api = yield* JsonPlaceholder
  const [todo, user] = yield* Effect.all([api.getTodo(1), api.getUser(1)])

  yield* Effect.log(`Todo #${todo.id}: ${todo.title}`)
  yield* Effect.log(`Assigned to: ${user.name} <${user.email}>`)
  yield* Effect.log(`Completed: ${todo.completed}`)
}).pipe(
  Effect.provide(JsonPlaceholderLive),
  Effect.catchTags({
    RequestError: (error) =>
      Effect.log(`Request failed for ${error.url}: ${String(error.cause)}`),
    InvalidStatusError: (error) =>
      Effect.log(`Request to ${error.url} returned status ${error.status}`),
    DecodeError: (error) =>
      Effect.log(`Could not decode JSON from ${error.url}: ${String(error.cause)}`)
  })
)

program.pipe(Effect.runPromise)
