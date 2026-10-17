import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import { Flag } from "effect/cli"

export class IntegrationSelectionError extends Data.TaggedError("IntegrationSelectionError")<{
  readonly reason: string
}> {
  get message(): string {
    return this.reason
  }
}

export const integrationFlags = {
  typescript: Flag.Boolean("typescript").pipe(
    Flag.withDefault(true),
    Flag.withDescription("Include the native TypeScript-Go integration")
  ),
  oxlint: Flag.Boolean("oxlint").pipe(
    Flag.withDefault(false),
    Flag.withDescription("Include the experimental Oxlint integration")
  )
}

export const ensureIntegrationSelected = (typescript: boolean, oxlint: boolean) =>
  typescript || oxlint
    ? Effect.void
    : Effect.fail(new IntegrationSelectionError({ reason: "Select at least one integration to manage." }))
