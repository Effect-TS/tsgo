// Parity with .repos/effect-language-service multiple.ts_AppLive snapshots
// verified. All four output formats (flat graph, nested graph, outline graph,
// quickinfo) are semantically identical to the .repos reference.
// Documented divergence that applies:
//   1. Alphabetical sorting of provides/requires type lists (extract.go)
// Divergences that do NOT apply (single-file test, no cross-file references):
//   2. Cross-file location annotations in flat output format (format.go)
//   3. "at in" double preposition fix in quickinfo for cross-file refs (format.go)
import { Effect, Layer, Context, pipe } from "effect"

class Database extends Context.Service<Database>()("Database", {
  make: Effect.succeed({})
}) {
  static Default = Layer.effect(this, this.make)
}

class UserRepository extends Context.Service<UserRepository>()("UserRepository", {
  make: Effect.as(Database.asEffect(), {})
}) {
  static Default = Layer.effect(this, this.make)
}

class EventsRepository extends Context.Service<EventsRepository>()("EventsRepository", {
  make: Effect.as(Database.asEffect(), {})
}) {
  static Default = Layer.effect(this, this.make)
}

class Analytics extends Context.Service<Analytics>()("Analytics", {
  make: Effect.succeed({})
}) {
  static Default = Layer.effect(this, this.make)
}

class UserService extends Context.Service<UserService>()("UserService", {
  make: Effect.as(Effect.andThen(UserRepository.asEffect(), Analytics.asEffect()), {})
}) {
  static Default = Layer.effect(this, this.make)
}

class EventService extends Context.Service<EventService>()("EventService", {
  make: Effect.as(Effect.andThen(EventsRepository.asEffect(), Analytics.asEffect()), {})
}) {
  static Default = Layer.effect(this, this.make)
}

class AppService extends Context.Service<AppService>()("AppService", {
  make: Effect.as(Effect.andThen(UserService.asEffect(), EventService.asEffect()), {})
}) {
  static Default = Layer.effect(this, this.make)
}

const DatabaseLive = Database.Default

export const AppLive = pipe(
  DatabaseLive,
  Layer.provideMerge(UserRepository.Default),
  Layer.provideMerge(EventsRepository.Default),
  Layer.merge(Analytics.Default),
  Layer.provideMerge(UserService.Default),
  Layer.provideMerge(EventService.Default),
  Layer.provideMerge(AppService.Default)
)
